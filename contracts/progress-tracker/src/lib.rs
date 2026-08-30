#![no_std]

mod rewards;
pub mod types;

use chainlearn_shared::ContractMetadata;
use soroban_sdk::{contract, contracterror, contractimpl, symbol_short, Address, Env, Symbol, Vec};
pub use types::{
    Course, LearnerStats, ProgressExport, ProgressInfo, ProgressTrackerDataKey, QuizResult,
    VersionedContractMetadata,
};

/// Sentinel meaning "no content hash set"; enrollment skips verification (#235).
const EMPTY_CONTENT_HASH: &str = "none";

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    AlreadyInitialized = 0,
}

/// On-chain learning progress tracker for ChainLearn.
///
/// Tracks learner enrollment, module completion, and quiz scores.
/// Provides progress calculation and credential eligibility checks.
#[contract]
pub struct ProgressTracker;

#[contractimpl]
impl ProgressTracker {
    /// Initialize the progress tracker with an admin.
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env
            .storage()
            .persistent()
            .has(&ProgressTrackerDataKey::Admin)
        {
            return Err(ContractError::AlreadyInitialized);
        }
        env.storage()
            .persistent()
            .set(&ProgressTrackerDataKey::Admin, &admin);
        env.storage().persistent().set(
            &ProgressTrackerDataKey::Metadata,
            &ContractMetadata::new(&env, "progress-tracker"),
        );
        // On-chain upgrade counter, separate from the crate's semantic
        // version above (#219). Starts at zero for a freshly initialized
        // contract; bumped whenever the contract is upgraded in place.
        env.storage()
            .persistent()
            .set(&ProgressTrackerDataKey::Version, &0u32);
        Ok(())
    }

    /// Get the contract's on-chain name, semantic version, and upgrade
    /// counter (#107, #219).
    ///
    /// Lets external tools (indexers, block explorers, upgrade tooling)
    /// identify which contract and release is deployed, and how many times
    /// it has been upgraded in place, without inferring it from behavior.
    pub fn contract_metadata(env: Env) -> VersionedContractMetadata {
        let metadata = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Metadata)
            .expect("not initialized");
        let version = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Version)
            .expect("not initialized");

        VersionedContractMetadata { metadata, version }
    }

    /// Get the contract's on-chain upgrade counter on its own (#219).
    ///
    /// Starts at `0` for a never-upgraded contract. progress-tracker has no
    /// in-place upgrade mechanism yet, so this only ever reads back the
    /// value set at `initialize()` today; it is queryable now so indexers
    /// and future upgrade tooling have a stable key to bump and read.
    pub fn contract_version(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Version)
            .expect("not initialized")
    }

    /// Register a new course with its modules and quizzes.
    ///
    /// # Arguments
    /// * `course_id` - Unique course identifier
    /// * `total_modules` - Number of modules in the course
    /// * `total_quizzes` - Number of quizzes in the course
    /// * `module_ids` - List of module identifiers (must not contain duplicates)
    /// * `quiz_ids` - List of valid quiz identifiers
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let env = Env::default();
    /// let admin = Address::generate(&env);
    /// let client = ProgressTrackerClient::new(&env, &contract_id);
    /// client.initialize(&admin);
    ///
    /// let mut modules = Vec::new(&env);
    /// modules.push_back(Symbol::new(&env, "basics"));
    /// modules.push_back(Symbol::new(&env, "ownership"));
    /// let mut quizzes = Vec::new(&env);
    /// quizzes.push_back(Symbol::new(&env, "quiz_1"));
    ///
    /// client.create_course(&Symbol::new(&env, "rust_101"), &2, &1, &modules, &quizzes);
    /// ```
    pub fn create_course(
        env: Env,
        course_id: Symbol,
        total_modules: u32,
        total_quizzes: u32,
        module_ids: Vec<Symbol>,
        quiz_ids: Vec<Symbol>,
    ) {
        Self::require_not_paused(&env);
        let admin: Address = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        if env
            .storage()
            .persistent()
            .has(&ProgressTrackerDataKey::Course(course_id.clone()))
        {
            panic!("course already exists");
        }

        if total_modules == 0 {
            panic!("total_modules must be greater than zero");
        }

        if total_modules > chainlearn_shared::MAX_MODULES_PER_COURSE {
            panic!("total_modules exceeds maximum modules per course");
        }

        if total_quizzes == 0 {
            panic!("total_quizzes must be greater than zero");
        }

        if module_ids.len() != total_modules {
            panic!("module_ids length must match total_modules");
        }

        if quiz_ids.len() != total_quizzes {
            panic!("quiz_ids length must match total_quizzes");
        }

        for i in 0..module_ids.len() {
            for j in (i + 1)..module_ids.len() {
                if module_ids.get(i) == module_ids.get(j) {
                    panic!("duplicate module_id found");
                }
            }
        }

        let course = Course {
            course_id: course_id.clone(),
            total_modules,
            total_quizzes,
            module_ids: module_ids.clone(),
            quiz_ids: quiz_ids.clone(),
            archived: false,
            // No content hash by default; set later via `set_course_content_hash` (#235).
            content_hash: Symbol::new(&env, EMPTY_CONTENT_HASH),
            prerequisites: Vec::new(&env),
        };

        env.storage()
            .persistent()
            .set(&ProgressTrackerDataKey::Course(course_id.clone()), &course);

        env.events().publish(
            (Symbol::new(&env, "course_created"),),
            (&course_id, total_modules, total_quizzes, module_ids.clone()),
        );
    }

    /// Enroll a learner in a course.
    ///
    /// # Arguments
    /// * `learner` - The learner address (must authorize)
    /// * `course_id` - The course to enroll in
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let learner = Address::generate(&env);
    /// env.mock_all_auths();
    /// client.enroll(&learner, &Symbol::new(&env, "rust_101"));
    /// let progress = client.get_progress(&learner, &Symbol::new(&env, "rust_101"));
    /// assert_eq!(progress.overall_progress, 0);
    /// assert!(!progress.eligible_for_credential);
    /// ```
    pub fn enroll(env: Env, learner: Address, course_id: Symbol) {
        Self::require_not_paused(&env);
        Self::enroll_checked(env, learner, course_id, None);
    }

    pub fn enroll_checked(
        env: Env,
        learner: Address,
        course_id: Symbol,
        expected_content_hash: Option<Symbol>,
    ) {
        learner.require_auth();

        // Verify course exists
        let course: Course = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Course(course_id.clone()))
            .expect("course not found");

        // Reject archived courses (#210)
        if course.archived {
            panic!("course is archived");
        }

        // Verify course has at least one module (#80)
        if course.total_modules == 0 {
            panic!("course has no modules");
        }

        // Enforce prerequisites: every prerequisite course must be completed
        // by this learner before enrollment is allowed (#231).
        for prerequisite in course.prerequisites.iter() {
            let prerequisite_progress: Option<ProgressInfo> =
                env.storage()
                    .persistent()
                    .get(&ProgressTrackerDataKey::Progress(
                        learner.clone(),
                        prerequisite.clone(),
                    ));

            match prerequisite_progress {
                Some(progress) if progress.eligible_for_credential => {}
                _ => panic!("prerequisite not completed"),
            }
        }

        // Content hash verification is optional (#235): it only runs when the
        // course has a hash set and the caller supplied one to check against.
        if let Some(expected) = expected_content_hash {
            let unset = Symbol::new(&env, EMPTY_CONTENT_HASH);
            if course.content_hash != unset && course.content_hash != expected {
                panic!("course content hash mismatch");
            }
        }

        // Check not already enrolled
        let key = ProgressTrackerDataKey::Progress(learner.clone(), course_id.clone());
        if env.storage().persistent().has(&key) {
            panic!("already enrolled");
        }

        let progress = ProgressInfo {
            enrolled_at: env.ledger().timestamp(),
            modules_completed_bitmap: 0,
            quizzes_submitted: 0,
            total_quiz_score: 0,
            overall_progress: 0,
            eligible_for_credential: false,
        };

        env.storage().persistent().set(&key, &progress);

        // Index the enrollment so learner-wide aggregates can be computed
        // without scanning every course in the contract (#232).
        let courses_key = ProgressTrackerDataKey::LearnerCourses(learner.clone());
        let mut courses: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&courses_key)
            .unwrap_or_else(|| Vec::new(&env));
        courses.push_back(course_id.clone());
        env.storage().persistent().set(&courses_key, &courses);

        env.events().publish(
            (symbol_short!("enrolled"),),
            (&learner, &course_id, progress.enrolled_at),
        );
    }

    /// Mark a module as completed for a learner.
    ///
    /// # Arguments
    /// * `learner` - The learner address (must authorize)
    /// * `course_id` - The course the module belongs to
    /// * `module_id` - The module to mark complete
    ///
    /// # Examples
    ///
    /// ```ignore
    /// env.mock_all_auths();
    /// let learner = Address::generate(&env);
    /// let course_id = Symbol::new(&env, "rust_101");
    /// client.enroll(&learner, &course_id);
    /// client.complete_module(&learner, &course_id, &Symbol::new(&env, "basics"));
    /// let progress = client.get_progress(&learner, &course_id);
    /// assert!(progress.overall_progress > 0);
    /// ```
    pub fn complete_module(env: Env, learner: Address, course_id: Symbol, module_id: Symbol) {
        Self::require_not_paused(&env);
        learner.require_auth();

        // Verify enrollment
        let mut progress: ProgressInfo = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Progress(
                learner.clone(),
                course_id.clone(),
            ))
            .expect("not enrolled");

        let course: Course = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Course(course_id.clone()))
            .expect("course not found");

        Self::complete_module_in_place(
            &env,
            &learner,
            &course_id,
            &course,
            &mut progress,
            module_id,
        );

        env.storage().persistent().set(
            &ProgressTrackerDataKey::Progress(learner.clone(), course_id.clone()),
            &progress,
        );
    }

    /// Complete several modules for a learner in one call (#220).
    ///
    /// Reuses the exact same per-module validation and completion logic as
    /// [`Self::complete_module`] via [`Self::complete_module_in_place`], applied
    /// once per entry in `module_ids`, in order.
    ///
    /// Atomic: each module is validated in turn (must exist in the course,
    /// must not already be completed, and its predecessor in course order
    /// must already be completed), and any failure panics immediately. Since
    /// Soroban transactions revert all storage writes on panic, a single
    /// invalid module id anywhere in the batch aborts the whole call --
    /// nothing from the batch is partially applied. This matches the
    /// ordered, sequential nature of module completion (contrast with
    /// [`Self::batch_submit_quiz_score`], where quizzes are independent
    /// facts and a bad entry is skipped rather than aborting the batch).
    ///
    /// # Arguments
    /// * `learner` - The learner address (must authorize)
    /// * `course_id` - The course the modules belong to
    /// * `module_ids` - The modules to mark complete, in the order to apply them
    ///
    /// # Examples
    ///
    /// ```ignore
    /// env.mock_all_auths();
    /// let mut modules = Vec::new(&env);
    /// modules.push_back(Symbol::new(&env, "mod_1"));
    /// modules.push_back(Symbol::new(&env, "mod_2"));
    /// client.batch_complete_module(&learner, &course_id, &modules);
    /// ```
    ///
    /// # Panics
    /// * If the learner is not enrolled in the course
    /// * If any module id does not exist in the course
    /// * If any module is already completed
    /// * If any module's predecessor in course order has not been completed
    pub fn batch_complete_module(
        env: Env,
        learner: Address,
        course_id: Symbol,
        module_ids: Vec<Symbol>,
    ) {
        Self::require_not_paused(&env);
        learner.require_auth();

        let mut progress: ProgressInfo = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Progress(
                learner.clone(),
                course_id.clone(),
            ))
            .expect("not enrolled");

        let course: Course = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Course(course_id.clone()))
            .expect("course not found");

        for module_id in module_ids.iter() {
            Self::complete_module_in_place(
                &env,
                &learner,
                &course_id,
                &course,
                &mut progress,
                module_id,
            );
        }

        env.storage().persistent().set(
            &ProgressTrackerDataKey::Progress(learner.clone(), course_id.clone()),
            &progress,
        );
    }

    /// Shared core of [`Self::complete_module`] and [`Self::batch_complete_module`]:
    /// validates and applies a single module completion against an
    /// already-loaded `course`/`progress` pair, mutating `progress` in place
    /// and publishing the same events `complete_module` always has. Callers
    /// are responsible for the single storage write of `progress` once all
    /// modules in a call have been applied.
    fn complete_module_in_place(
        env: &Env,
        learner: &Address,
        course_id: &Symbol,
        course: &Course,
        progress: &mut ProgressInfo,
        module_id: Symbol,
    ) {
        // Check not already completed
        let completed_key = ProgressTrackerDataKey::ModuleCompleted(
            learner.clone(),
            course_id.clone(),
            module_id.clone(),
        );
        if env.storage().persistent().has(&completed_key) {
            panic!("module already completed");
        }

        // Verify module exists in course and get its index
        let mut module_index: Option<u32> = None;
        for (i, m) in course.module_ids.iter().enumerate() {
            if m == module_id {
                module_index = Some(i as u32);
                break;
            }
        }
        let idx = module_index.expect("module not found in course");
        if idx >= 64 {
            panic!("module index exceeds bitmap capacity");
        }

        // Enforce sequential ordering: module at index N requires module N-1 completed
        if idx > 0 {
            let prev_module = course.module_ids.get(idx - 1).unwrap();
            let prev_key = ProgressTrackerDataKey::ModuleCompleted(
                learner.clone(),
                course_id.clone(),
                prev_module.clone(),
            );
            if !env.storage().persistent().has(&prev_key) {
                panic!("previous module not completed");
            }
        }

        // Mark module as completed
        env.storage().persistent().set(&completed_key, &true);
        progress.modules_completed_bitmap |= 1 << idx;

        let was_eligible = progress.eligible_for_credential;

        progress.overall_progress = rewards::calculate_progress(course, progress);
        progress.eligible_for_credential = rewards::is_eligible_for_credential(course, progress);

        env.events().publish(
            (Symbol::new(env, "module_completed"),),
            (learner, course_id, &module_id, progress.overall_progress),
        );

        // Notify indexers the moment eligibility flips to true, instead of
        // requiring them to poll get_progress (#96).
        if !was_eligible && progress.eligible_for_credential {
            env.events().publish(
                (Symbol::new(env, "credential_eligible"),),
                (learner, course_id),
            );
        }
    }

    /// Submit a quiz score for a learner.
    ///
    /// # Arguments
    /// * `learner` - The learner address (must authorize)
    /// * `course_id` - The course the quiz belongs to
    /// * `quiz_id` - The quiz identifier
    /// * `score` - The score achieved (0-100)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// env.mock_all_auths();
    /// let learner = Address::generate(&env);
    /// let course_id = Symbol::new(&env, "rust_101");
    /// client.enroll(&learner, &course_id);
    /// client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &85);
    /// let progress = client.get_progress(&learner, &course_id);
    /// assert_eq!(progress.quizzes_submitted, 1);
    /// assert_eq!(progress.total_quiz_score, 85);
    /// ```
    pub fn submit_quiz_score(
        env: Env,
        learner: Address,
        course_id: Symbol,
        quiz_id: Symbol,
        score: u32,
    ) {
        Self::require_not_paused(&env);
        learner.require_auth();

        if score > chainlearn_shared::MAX_QUIZ_SCORE {
            panic!("score exceeds maximum");
        }

        // Verify enrollment
        let mut progress: ProgressInfo = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Progress(
                learner.clone(),
                course_id.clone(),
            ))
            .expect("not enrolled");

        // Check not already submitted
        let quiz_key =
            ProgressTrackerDataKey::QuizResult(learner.clone(), course_id.clone(), quiz_id.clone());
        if env.storage().persistent().has(&quiz_key) {
            panic!("quiz already submitted");
        }

        // Verify course and quiz_id
        let course: Course = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Course(course_id.clone()))
            .expect("course not found");

        if !course.quiz_ids.contains(&quiz_id) {
            panic!("quiz_id not found in course");
        }

        // The quiz result is stored once, under its own key (#83). ProgressInfo
        // keeps only the aggregates needed for progress and eligibility, so the
        // result is never duplicated into a growing Vec.
        let result = QuizResult {
            quiz_id: quiz_id.clone(),
            course_id: course_id.clone(),
            score,
            submitted_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&quiz_key, &result);

        progress.quizzes_submitted += 1;
        progress.total_quiz_score += score as u64;

        let was_eligible = progress.eligible_for_credential;

        // Recalculate from the updated in-memory aggregates, so everything is
        // known before the single storage write below.
        progress.overall_progress = rewards::calculate_progress(&course, &progress);
        progress.eligible_for_credential = rewards::is_eligible_for_credential(&course, &progress);

        // Single write with all updated fields
        env.storage().persistent().set(
            &ProgressTrackerDataKey::Progress(learner.clone(), course_id.clone()),
            &progress,
        );

        env.events().publish(
            (Symbol::new(&env, "quiz_submitted"),),
            (&learner, &course_id, &quiz_id, score),
        );

        // Notify indexers the moment eligibility flips to true, instead of
        // requiring them to poll get_progress (#96).
        if !was_eligible && progress.eligible_for_credential {
            env.events().publish(
                (Symbol::new(&env, "credential_eligible"),),
                (&learner, &course_id),
            );
        }
    }

    /// Retake a quiz with a higher score (#234).
    ///
    /// Quiz scores were final once submitted, so a learner who under-performed
    /// could never improve their course average or reach credential
    /// eligibility. A retake replaces the stored [`QuizResult`] in place and
    /// adjusts the running aggregates by the difference, so the quiz is still
    /// counted exactly once.
    ///
    /// The new score must beat the recorded one: a retake can only ever move a
    /// learner's average up, so a learner cannot lower their own score, and
    /// nobody can replay an old submission to undo an improvement.
    ///
    /// # Arguments
    /// * `learner` - The learner address (must authorize)
    /// * `course_id` - The course the quiz belongs to
    /// * `quiz_id` - The quiz being retaken
    /// * `new_score` - The improved score (0-100, strictly greater than the
    ///   score already recorded)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// env.mock_all_auths();
    /// client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &55);
    /// client.retake_quiz(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &90);
    /// assert_eq!(client.get_course_score(&learner, &course_id), 90);
    /// ```
    ///
    /// # Panics
    /// * If `new_score` exceeds `MAX_QUIZ_SCORE`
    /// * If the learner is not enrolled in the course
    /// * If the quiz has not been submitted yet
    /// * If `new_score` is not strictly higher than the recorded score
    pub fn retake_quiz(
        env: Env,
        learner: Address,
        course_id: Symbol,
        quiz_id: Symbol,
        new_score: u32,
    ) {
        learner.require_auth();

        if new_score > chainlearn_shared::MAX_QUIZ_SCORE {
            panic!("score exceeds maximum");
        }

        let mut progress: ProgressInfo = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Progress(
                learner.clone(),
                course_id.clone(),
            ))
            .expect("not enrolled");

        // A retake only makes sense for a quiz that was actually taken; a
        // first attempt still goes through submit_quiz_score.
        let quiz_key =
            ProgressTrackerDataKey::QuizResult(learner.clone(), course_id.clone(), quiz_id.clone());
        let mut result: QuizResult = env
            .storage()
            .persistent()
            .get(&quiz_key)
            .expect("quiz not submitted");

        if new_score <= result.score {
            panic!("new score must be higher");
        }

        let course: Course = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Course(course_id.clone()))
            .expect("course not found");

        let previous_score = result.score;

        // Replace the result in place and move the running sum by the
        // difference, so quizzes_submitted -- and therefore the average's
        // divisor -- is unchanged by a retake.
        result.score = new_score;
        result.submitted_at = env.ledger().timestamp();
        env.storage().persistent().set(&quiz_key, &result);

        progress.total_quiz_score += (new_score - previous_score) as u64;

        let was_eligible = progress.eligible_for_credential;

        progress.overall_progress = rewards::calculate_progress(&course, &progress);
        progress.eligible_for_credential = rewards::is_eligible_for_credential(&course, &progress);

        env.storage().persistent().set(
            &ProgressTrackerDataKey::Progress(learner.clone(), course_id.clone()),
            &progress,
        );

        env.events().publish(
            (Symbol::new(&env, "quiz_retaken"),),
            (&learner, &course_id, &quiz_id, previous_score, new_score),
        );

        // Notify indexers the moment eligibility flips to true, instead of
        // requiring them to poll get_progress (#96).
        if !was_eligible && progress.eligible_for_credential {
            env.events().publish(
                (Symbol::new(&env, "credential_eligible"),),
                (&learner, &course_id),
            );
        }
    }

    /// Get a learner's progress in a course.
    ///
    /// Read-only: the arguments are moved straight into the storage key, so
    /// nothing is cloned. (Contract entry points cannot take `&Address` --
    /// `#[contractimpl]` rejects reference arguments -- so avoiding clones is
    /// what keeps a read cheap.)
    ///
    /// # Arguments
    /// * `learner` - The learner address
    /// * `course_id` - The course identifier
    ///
    /// # Returns
    /// The `ProgressInfo` for the learner in the given course.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// env.mock_all_auths();
    /// let learner = Address::generate(&env);
    /// let course_id = Symbol::new(&env, "rust_101");
    /// client.enroll(&learner, &course_id);
    /// client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
    /// client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &85);
    /// let progress = client.get_progress(&learner, &course_id);
    /// assert!(progress.overall_progress > 0);
    /// assert_eq!(progress.quizzes_submitted, 1);
    /// ```
    pub fn get_progress(env: Env, learner: Address, course_id: Symbol) -> ProgressInfo {
        env.storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Progress(learner, course_id))
            .expect("not enrolled")
    }

    /// Get just a learner's progress percentage in a course (#233).
    ///
    /// Frontends that only render a progress bar do not need the whole
    /// [`ProgressInfo`], so this returns the stored `overall_progress` on its
    /// own. The value is maintained on every write that can change it
    /// (`complete_module`, `submit_quiz_score`, `retake_quiz`), so this is a
    /// single storage read with no recomputation and no state change.
    ///
    /// # Arguments
    /// * `learner` - The learner address
    /// * `course_id` - The course identifier
    ///
    /// # Returns
    /// The progress percentage (0-100).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// env.mock_all_auths();
    /// client.enroll(&learner, &course_id);
    /// assert_eq!(client.get_completion_percentage(&learner, &course_id), 0);
    /// ```
    ///
    /// # Panics
    /// * If the learner is not enrolled in the course
    pub fn get_completion_percentage(env: Env, learner: Address, course_id: Symbol) -> u32 {
        let progress: ProgressInfo = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Progress(learner, course_id))
            .expect("not enrolled");

        progress.overall_progress
    }

    /// Export a learner's complete progress for a course in a single call (#196).
    ///
    /// Aggregates data that otherwise lives under several storage keys —
    /// enrollment, every submitted quiz result, and the [`ProgressInfo`]
    /// aggregates — into one [`ProgressExport`], for external tools that need
    /// the full picture without walking each quiz individually via
    /// [`Self::get_quiz_score`].
    ///
    /// # Arguments
    /// * `learner` - The learner address
    /// * `course_id` - The course to export progress for
    ///
    /// # Panics
    /// * If the learner is not enrolled in the course
    /// * If the course does not exist
    pub fn export_progress(env: Env, learner: Address, course_id: Symbol) -> ProgressExport {
        let progress: ProgressInfo = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Progress(
                learner.clone(),
                course_id.clone(),
            ))
            .expect("not enrolled");

        let course: Course = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Course(course_id.clone()))
            .expect("course not found");

        let mut quiz_scores = Vec::new(&env);
        for quiz_id in course.quiz_ids.iter() {
            let key = ProgressTrackerDataKey::QuizResult(
                learner.clone(),
                course_id.clone(),
                quiz_id.clone(),
            );
            let existing: Option<QuizResult> = env.storage().persistent().get(&key);
            if let Some(result) = existing {
                quiz_scores.push_back(result);
            }
        }

        ProgressExport {
            enrolled: true,
            enrolled_at: progress.enrolled_at,
            modules_completed_bitmap: progress.modules_completed_bitmap,
            total_modules: course.total_modules,
            quiz_scores,
            quizzes_submitted: progress.quizzes_submitted,
            total_quiz_score: progress.total_quiz_score,
            overall_progress: progress.overall_progress,
            eligible_for_credential: progress.eligible_for_credential,
        }
    }

    /// Get a verified quiz score for a learner.
    ///
    /// Returns the score if the quiz was submitted, or panics if not found.
    /// Used by the token contract to verify scores before minting rewards.
    /// Read-only and clone-free, like [`Self::get_progress`].
    ///
    /// # Arguments
    /// * `learner` - The learner address
    /// * `course_id` - The course the quiz belongs to
    /// * `quiz_id` - The quiz identifier
    ///
    /// # Returns
    /// The verified quiz score (0-100).
    pub fn get_quiz_score(env: Env, learner: Address, course_id: Symbol, quiz_id: Symbol) -> u32 {
        let result: QuizResult = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::QuizResult(
                learner,
                course_id.clone(),
                quiz_id.clone(),
            ))
            .expect("quiz not submitted");

        let course: Course = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Course(course_id.clone()))
            .expect("course not found");

        if !course.quiz_ids.contains(&quiz_id) {
            panic!("quiz_id not found in course");
        }

        if result.course_id != course_id {
            panic!("course_id does not match quiz");
        }

        result.score
    }

    /// Get a learner's verified course score: the average across every quiz
    /// they have submitted for the course, floored to a whole number.
    ///
    /// This is the on-chain source of truth for the score recorded on a
    /// credential. The credential contract calls it before minting so a caller
    /// cannot claim a score the learner never earned (#34).
    ///
    /// The aggregates it divides (`total_quiz_score`, `quizzes_submitted`) are
    /// already maintained on every submission, so this stays a single read.
    ///
    /// # Arguments
    /// * `learner` - The learner address
    /// * `course_id` - The course identifier
    ///
    /// # Returns
    /// The average submitted quiz score (0-100).
    ///
    /// # Panics
    /// * If the learner is not enrolled in the course
    /// * If the learner has not submitted any quiz for the course
    pub fn get_course_score(env: Env, learner: Address, course_id: Symbol) -> u32 {
        let progress: ProgressInfo = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Progress(learner, course_id))
            .expect("not enrolled");

        if progress.quizzes_submitted == 0 {
            panic!("no quizzes submitted");
        }

        // Both aggregates are bounded (score <= MAX_QUIZ_SCORE per quiz), so the
        // average always fits back into u32.
        (progress.total_quiz_score / progress.quizzes_submitted as u64) as u32
    }

    /// Check if a learner is eligible for a credential.
    ///
    /// `eligible_for_credential` is kept up to date on every write that could
    /// change it (`complete_module`, `submit_quiz_score`), so the stored
    /// `ProgressInfo` already carries the answer -- no need to re-derive it
    /// from `Course` and the `ModuleCompleted` keys on every read (#98).
    ///
    /// # Arguments
    /// * `learner` - The learner address
    /// * `course_id` - The course identifier
    ///
    /// # Returns
    /// `true` if the learner qualifies for a credential.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// env.mock_all_auths();
    /// let learner = Address::generate(&env);
    /// let course_id = Symbol::new(&env, "rust_101");
    /// client.enroll(&learner, &course_id);
    /// assert!(!client.is_eligible_for_credential(&learner, &course_id));
    ///
    /// client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
    /// client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
    /// client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &75);
    /// client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &75);
    /// assert!(client.is_eligible_for_credential(&learner, &course_id));
    /// ```
    ///
    /// # Panics
    /// * If the learner is not enrolled in the course
    pub fn is_eligible_for_credential(env: Env, learner: Address, course_id: Symbol) -> bool {
        let progress: ProgressInfo = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Progress(learner, course_id))
            .expect("not enrolled");

        progress.eligible_for_credential
    }

    /// Get course configuration.
    ///
    /// # Arguments
    /// * `course_id` - The course identifier
    pub fn get_course(env: Env, course_id: Symbol) -> Course {
        env.storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Course(course_id))
            .expect("course not found")
    }

    /// Archive a course, preventing new enrollments (#210). Admin only.
    ///
    /// Archived courses preserve all existing progress but reject new enrollments.
    ///
    /// # Arguments
    /// * `course_id` - The course to archive
    pub fn archive_course(env: Env, course_id: Symbol) {
        Self::require_not_paused(&env);
        let admin: Address = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        let mut course: Course = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Course(course_id.clone()))
            .expect("course not found");

        if course.archived {
            panic!("course already archived");
        }

        course.archived = true;
        env.storage()
            .persistent()
            .set(&ProgressTrackerDataKey::Course(course_id.clone()), &course);

        env.events()
            .publish((Symbol::new(&env, "course_archived"),), (&course_id,));
    }

    /// Set or update the content hash for a course. Admin only (#235).
    ///
    /// The hash lets clients verify that off-chain course content matches what
    /// the course was published with. Setting it to `none` disables
    /// verification again.
    ///
    /// # Arguments
    /// * `course_id` - The course to update
    /// * `content_hash` - Hash of the course content, or `none` to unset
    pub fn set_course_content_hash(env: Env, course_id: Symbol, content_hash: Symbol) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        let mut course: Course = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Course(course_id.clone()))
            .expect("course not found");

        course.content_hash = content_hash.clone();
        env.storage()
            .persistent()
            .set(&ProgressTrackerDataKey::Course(course_id.clone()), &course);

        env.events().publish(
            (Symbol::new(&env, "content_hash_set"),),
            (&course_id, &content_hash),
        );
    }

    /// Returns the content hash for a course (#235).
    ///
    /// Returns the `none` sentinel when no hash has been set.
    ///
    /// # Arguments
    /// * `course_id` - The course identifier
    pub fn get_course_content_hash(env: Env, course_id: Symbol) -> Symbol {
        let course: Course = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Course(course_id))
            .expect("course not found");
        course.content_hash
    }

    /// Set the courses that must be completed before enrolling in `course_id` (#231).
    /// Admin only.
    ///
    /// Replaces any previously configured prerequisites. Passing an empty list
    /// clears them. Existing enrollments are unaffected -- prerequisites are
    /// only checked at [`Self::enroll`] time.
    ///
    /// # Arguments
    /// * `course_id` - The course to configure
    /// * `prerequisites` - Course IDs that must be completed first
    ///
    /// # Panics
    /// * If the course does not exist
    /// * If any prerequisite course does not exist
    /// * If a course is listed as its own prerequisite
    /// * If the list contains duplicates
    ///
    /// # Examples
    ///
    /// ```ignore
    /// env.mock_all_auths();
    /// let mut prereqs = Vec::new(&env);
    /// prereqs.push_back(Symbol::new(&env, "rust_101"));
    /// client.set_prerequisites(&Symbol::new(&env, "rust_201"), &prereqs);
    /// assert_eq!(client.get_prerequisites(&Symbol::new(&env, "rust_201")), prereqs);
    /// ```
    pub fn set_prerequisites(env: Env, course_id: Symbol, prerequisites: Vec<Symbol>) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        let mut course: Course = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Course(course_id.clone()))
            .expect("course not found");

        for i in 0..prerequisites.len() {
            let prerequisite = prerequisites.get(i).unwrap();

            if prerequisite == course_id {
                panic!("course cannot be its own prerequisite");
            }

            if !env
                .storage()
                .persistent()
                .has(&ProgressTrackerDataKey::Course(prerequisite.clone()))
            {
                panic!("prerequisite course not found");
            }

            for j in (i + 1)..prerequisites.len() {
                if prerequisites.get(j) == Some(prerequisite.clone()) {
                    panic!("duplicate prerequisite found");
                }
            }
        }

        course.prerequisites = prerequisites.clone();
        env.storage()
            .persistent()
            .set(&ProgressTrackerDataKey::Course(course_id.clone()), &course);

        env.events().publish(
            (Symbol::new(&env, "prerequisites_set"),),
            (&course_id, prerequisites),
        );
    }

    /// Get the prerequisite courses for a course (#231).
    ///
    /// Returns an empty list when the course has no prerequisites.
    ///
    /// # Arguments
    /// * `course_id` - The course identifier
    ///
    /// # Panics
    /// * If the course does not exist
    pub fn get_prerequisites(env: Env, course_id: Symbol) -> Vec<Symbol> {
        let course: Course = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Course(course_id))
            .expect("course not found");

        course.prerequisites
    }

    /// Get every course a learner has enrolled in, in enrollment order (#232).
    ///
    /// Returns an empty list for a learner who has never enrolled in anything.
    ///
    /// # Arguments
    /// * `learner` - The learner address
    pub fn get_learner_courses(env: Env, learner: Address) -> Vec<Symbol> {
        env.storage()
            .persistent()
            .get(&ProgressTrackerDataKey::LearnerCourses(learner))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Get a learner's aggregate statistics across every enrolled course (#232).
    ///
    /// Dashboards need totals -- courses enrolled, courses completed, average
    /// score, rewards earned -- that otherwise require one `get_progress` call
    /// per course. This walks the learner's course index once and returns
    /// everything in a single [`LearnerStats`], so no pagination or repeated
    /// round trips are needed.
    ///
    /// A learner who has never enrolled gets an all-zero result rather than a
    /// panic, so callers can render a new learner without a special case.
    ///
    /// # Arguments
    /// * `learner` - The learner address
    ///
    /// # Examples
    ///
    /// ```ignore
    /// env.mock_all_auths();
    /// client.enroll(&learner, &course_id);
    /// client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);
    ///
    /// let stats = client.get_learner_stats(&learner);
    /// assert_eq!(stats.courses_enrolled, 1);
    /// assert_eq!(stats.average_score, 80);
    /// ```
    pub fn get_learner_stats(env: Env, learner: Address) -> LearnerStats {
        let courses: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::LearnerCourses(learner.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        let mut courses_completed = 0u32;
        let mut total_quizzes_submitted = 0u32;
        let mut total_quiz_score = 0u64;

        for course_id in courses.iter() {
            let progress: ProgressInfo = env
                .storage()
                .persistent()
                .get(&ProgressTrackerDataKey::Progress(
                    learner.clone(),
                    course_id.clone(),
                ))
                .expect("not enrolled");

            if progress.eligible_for_credential {
                courses_completed += 1;
            }
            total_quizzes_submitted += progress.quizzes_submitted;
            total_quiz_score += progress.total_quiz_score;
        }

        // Every score is bounded by MAX_QUIZ_SCORE, so the average always fits
        // back into u32.
        let average_score = if total_quizzes_submitted == 0 {
            0
        } else {
            (total_quiz_score / total_quizzes_submitted as u64) as u32
        };

        LearnerStats {
            courses_enrolled: courses.len(),
            courses_completed,
            total_quizzes_submitted,
            total_quiz_score,
            average_score,
            total_rewards_earned: total_quiz_score as i128
                * chainlearn_shared::BASE_REWARD_PER_POINT,
        }
    }

    /// Check whether a course has been registered via `create_course` (#108).
    ///
    /// A cheap existence check -- unlike `get_course`, it never deserializes
    /// the `Course` struct -- so other contracts (e.g. credential-nft) can
    /// validate a `course_id` before acting on it.
    ///
    /// # Arguments
    /// * `course_id` - The course identifier
    pub fn course_exists(env: Env, course_id: Symbol) -> bool {
        env.storage()
            .persistent()
            .has(&ProgressTrackerDataKey::Course(course_id))
    }

    // ── Emergency Pause (#189) ────────────────────────────────────────────

    fn is_paused(env: &Env) -> bool {
        env.storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Paused)
            .unwrap_or(false)
    }

    fn require_not_paused(env: &Env) {
        if Self::is_paused(env) {
            panic!("contract is paused");
        }
    }

    /// Pause all state-changing operations. Admin only.
    pub fn emergency_pause(env: Env) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&ProgressTrackerDataKey::Paused, &true);
        // We omit events here to avoid adding it to events.rs
    }

    /// Unpause state-changing operations. Admin only.
    pub fn unpause(env: Env) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&ProgressTrackerDataKey::Paused, &false);
    }

    /// Returns the admin address.
    pub fn admin(env: Env) -> Address {
        env.storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Admin)
            .expect("not initialized")
    }

    /// Transfer admin rights to a new address.
    ///
    /// # Arguments
    /// * `new_admin` - The new admin address
    pub fn transfer_admin(env: Env, new_admin: Address) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        let zero_address = Address::from_string(&soroban_sdk::String::from_str(
            &env,
            "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
        ));
        if new_admin == zero_address {
            panic!("cannot transfer admin to zero address");
        }

        env.storage()
            .persistent()
            .set(&ProgressTrackerDataKey::Admin, &new_admin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Events as _, Ledger as _},
        vec, IntoVal,
    };

    fn setup_contract(env: &Env) -> (Address, Address) {
        let admin = Address::generate(env);
        let contract_id = env.register_contract(None, ProgressTracker);
        let client = ProgressTrackerClient::new(env, &contract_id);
        client.initialize(&admin);
        (admin, contract_id)
    }

    fn create_test_course(env: &Env, client: &ProgressTrackerClient) -> Symbol {
        let course_id = Symbol::new(env, "rust_101");
        let mut module_ids = Vec::new(env);
        module_ids.push_back(Symbol::new(env, "mod_1"));
        module_ids.push_back(Symbol::new(env, "mod_2"));
        module_ids.push_back(Symbol::new(env, "mod_3"));
        let mut quiz_ids = Vec::new(env);
        quiz_ids.push_back(Symbol::new(env, "quiz_1"));
        quiz_ids.push_back(Symbol::new(env, "quiz_2"));
        client.create_course(&course_id, &3, &2, &module_ids, &quiz_ids);
        course_id
    }

    // ── Issue #107: initialize() stores contract name/version metadata ──────

    #[test]
    fn test_initialize_stores_contract_metadata() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let metadata = client.contract_metadata();
        assert_eq!(
            metadata.metadata.name,
            soroban_sdk::String::from_str(&env, "progress-tracker")
        );
        assert_eq!(
            metadata.metadata.version,
            soroban_sdk::String::from_str(&env, chainlearn_shared::CONTRACT_VERSION)
        );
    }

    // ── Issue #219: on-chain upgrade counter ─────────────────────────────

    #[test]
    fn test_contract_version_starts_at_zero() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        assert_eq!(client.contract_version(), 0);
    }

    #[test]
    fn test_contract_metadata_includes_version() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let metadata = client.contract_metadata();
        assert_eq!(metadata.version, 0);
        assert_eq!(metadata.version, client.contract_version());
    }

    // ── Issue #108: course_exists lets other contracts validate course_id ───

    #[test]
    fn test_course_exists() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        assert!(!client.course_exists(&Symbol::new(&env, "ghost_course")));

        let course_id = create_test_course(&env, &client);
        assert!(client.course_exists(&course_id));
    }

    // ── Issue #235: course content hash verification ─────────────────────

    #[test]
    fn test_course_content_hash_defaults_to_unset() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);

        assert_eq!(
            client.get_course_content_hash(&course_id),
            Symbol::new(&env, EMPTY_CONTENT_HASH)
        );
    }

    #[test]
    fn test_set_and_query_course_content_hash() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let hash = Symbol::new(&env, "abc123");

        client.set_course_content_hash(&course_id, &hash);

        assert_eq!(client.get_course_content_hash(&course_id), hash);
        assert_eq!(client.get_course(&course_id).content_hash, hash);
    }

    #[test]
    fn test_course_content_hash_can_be_updated() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);

        client.set_course_content_hash(&course_id, &Symbol::new(&env, "v1"));
        client.set_course_content_hash(&course_id, &Symbol::new(&env, "v2"));

        assert_eq!(
            client.get_course_content_hash(&course_id),
            Symbol::new(&env, "v2")
        );
    }

    #[test]
    fn test_enroll_without_hash_is_unaffected_by_verification() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        client.set_course_content_hash(&course_id, &Symbol::new(&env, "abc123"));

        // Plain `enroll` never verifies, so a set hash does not block it.
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);

        assert_eq!(
            client.get_progress(&learner, &course_id).overall_progress,
            0
        );
    }

    #[test]
    fn test_enroll_checked_accepts_matching_hash() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let hash = Symbol::new(&env, "abc123");
        client.set_course_content_hash(&course_id, &hash);

        let learner = Address::generate(&env);
        client.enroll_checked(&learner, &course_id, &Some(hash));

        assert_eq!(
            client.get_progress(&learner, &course_id).overall_progress,
            0
        );
    }

    #[test]
    #[should_panic(expected = "course content hash mismatch")]
    fn test_enroll_checked_rejects_mismatched_hash() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        client.set_course_content_hash(&course_id, &Symbol::new(&env, "abc123"));

        let learner = Address::generate(&env);
        client.enroll_checked(&learner, &course_id, &Some(Symbol::new(&env, "wrong")));
    }

    #[test]
    fn test_enroll_checked_skips_verification_when_hash_unset() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);

        // Course has no hash set, so verification is skipped even though the
        // caller supplied one -- verification is optional.
        let learner = Address::generate(&env);
        client.enroll_checked(&learner, &course_id, &Some(Symbol::new(&env, "anything")));

        assert_eq!(
            client.get_progress(&learner, &course_id).overall_progress,
            0
        );
    }

    #[test]
    fn test_enroll_checked_with_none_skips_verification() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        client.set_course_content_hash(&course_id, &Symbol::new(&env, "abc123"));

        let learner = Address::generate(&env);
        client.enroll_checked(&learner, &course_id, &None);

        assert_eq!(
            client.get_progress(&learner, &course_id).overall_progress,
            0
        );
    }

    // ── Issue #34: verified course score ─────────────────────────────────

    #[test]
    fn test_get_course_score_single_quiz() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &73);

        assert_eq!(client.get_course_score(&learner, &course_id), 73);
    }

    #[test]
    fn test_get_course_score_averages_submitted_quizzes() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &90);

        assert_eq!(client.get_course_score(&learner, &course_id), 85);
    }

    #[test]
    fn test_get_course_score_floors_the_average() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);
        // (80 + 91) / 2 = 85.5 -> 85
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &91);

        assert_eq!(client.get_course_score(&learner, &course_id), 85);
    }

    #[test]
    fn test_get_course_score_is_per_learner() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let strong = Address::generate(&env);
        let weak = Address::generate(&env);

        client.enroll(&strong, &course_id);
        client.enroll(&weak, &course_id);
        client.submit_quiz_score(&strong, &course_id, &Symbol::new(&env, "quiz_1"), &95);
        client.submit_quiz_score(&weak, &course_id, &Symbol::new(&env, "quiz_1"), &55);

        assert_eq!(client.get_course_score(&strong, &course_id), 95);
        assert_eq!(client.get_course_score(&weak, &course_id), 55);
    }

    #[test]
    #[should_panic(expected = "no quizzes submitted")]
    fn test_get_course_score_without_submissions_panics() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);

        // Enrolled but no quiz taken — there is no score to verify against.
        client.get_course_score(&learner, &course_id);
    }

    #[test]
    #[should_panic(expected = "not enrolled")]
    fn test_get_course_score_for_unenrolled_learner_panics() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let stranger = Address::generate(&env);

        client.get_course_score(&stranger, &course_id);
    }

    #[test]
    fn test_enroll_and_get_progress() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        let progress = client.get_progress(&learner, &course_id);

        assert_eq!(progress.overall_progress, 0);
        assert!(!progress.eligible_for_credential);
    }

    #[test]
    fn test_complete_module() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));

        let progress = client.get_progress(&learner, &course_id);
        assert!(progress.overall_progress > 0);
    }

    #[test]
    fn test_submit_quiz_score() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &85);

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.quizzes_submitted, 1);
        assert_eq!(progress.total_quiz_score, 85);
        assert_eq!(
            client.get_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1")),
            85
        );
    }

    #[test]
    fn test_export_progress() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &85);

        let export = client.export_progress(&learner, &course_id);

        assert!(export.enrolled);
        assert_eq!(export.total_modules, 3);
        assert_eq!(export.modules_completed_bitmap, 1);
        assert_eq!(export.quizzes_submitted, 1);
        assert_eq!(export.total_quiz_score, 85);
        assert_eq!(export.quiz_scores.len(), 1);
        assert_eq!(
            export.quiz_scores.get(0).unwrap().quiz_id,
            Symbol::new(&env, "quiz_1")
        );
        assert_eq!(export.quiz_scores.get(0).unwrap().score, 85);
        assert!(!export.eligible_for_credential);

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(export.overall_progress, progress.overall_progress);
        assert_eq!(export.enrolled_at, progress.enrolled_at);
    }

    #[test]
    fn test_export_progress_omits_unsubmitted_quizzes() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &70);
        // quiz_2 is never submitted.

        let export = client.export_progress(&learner, &course_id);

        assert_eq!(export.quiz_scores.len(), 1);
        assert_eq!(
            export.quiz_scores.get(0).unwrap().quiz_id,
            Symbol::new(&env, "quiz_1")
        );
    }

    #[test]
    #[should_panic(expected = "not enrolled")]
    fn test_export_progress_not_enrolled() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.export_progress(&learner, &course_id);
    }

    #[test]
    fn test_eligibility_after_completion() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);

        // Complete all modules
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_3"));

        // Submit all quizzes with passing scores
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &70);

        let progress = client.get_progress(&learner, &course_id);
        assert!(progress.eligible_for_credential);
        assert_eq!(progress.overall_progress, 92);
    }

    #[test]
    #[should_panic(expected = "not enrolled")]
    fn test_get_progress_not_enrolled() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.get_progress(&learner, &course_id);
    }

    #[test]
    #[should_panic(expected = "already enrolled")]
    fn test_double_enroll() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.enroll(&learner, &course_id); // should panic
    }

    #[test]
    #[should_panic(expected = "course already exists")]
    fn test_create_course_rejects_duplicate() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);

        // Try to create a course with the same ID — should panic
        let mut module_ids = Vec::new(&env);
        module_ids.push_back(Symbol::new(&env, "mod_a"));
        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(Symbol::new(&env, "quiz_a"));
        client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
    }

    /// #83: a quiz result is written once, under its own key. ProgressInfo
    /// only carries the aggregates derived from it.
    #[test]
    fn test_quiz_result_not_duplicated_in_progress() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &60);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &90);

        // Each result is still retrievable from its own storage key.
        assert_eq!(
            client.get_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1")),
            60
        );
        assert_eq!(
            client.get_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2")),
            90
        );

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.quizzes_submitted, 2);
        assert_eq!(progress.total_quiz_score, 150);
        // Average 75 → 75 * 30 / 100 = 22, no modules completed.
        assert_eq!(progress.overall_progress, 22);
    }

    /// #86: eligibility requires enrollment, checked before course data.
    #[test]
    #[should_panic(expected = "not enrolled")]
    fn test_is_eligible_rejects_unenrolled_learner() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.is_eligible_for_credential(&learner, &course_id);
    }

    #[test]
    fn test_is_eligible_for_enrolled_learner() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        assert!(!client.is_eligible_for_credential(&learner, &course_id));

        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_3"));
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &70);

        assert!(client.is_eligible_for_credential(&learner, &course_id));
    }

    /// #94: course_created event must include module_ids so indexers can
    /// reconstruct the full course structure without extra storage reads.
    #[test]
    fn test_create_course_event_emitted() {
        let env = Env::default();
        env.mock_all_auths();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let course_id = Symbol::new(&env, "sol_201");
        let mut module_ids = Vec::new(&env);
        module_ids.push_back(Symbol::new(&env, "mod_a"));
        module_ids.push_back(Symbol::new(&env, "mod_b"));
        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(Symbol::new(&env, "quiz_a"));

        client.create_course(&course_id, &2, &1, &module_ids, &quiz_ids);

        // Verify at least one event was emitted (the course_created event).
        // The event now carries (course_id, total_modules, total_quizzes, module_ids)
        // so indexers can reconstruct the full module list without additional reads.
        let events = env.events().all();
        assert!(!events.is_empty(), "course_created event must be emitted");
    }

    /// #85: get_quiz_score reads without cloning; an unsubmitted quiz panics.
    #[test]
    #[should_panic(expected = "quiz not submitted")]
    fn test_get_quiz_score_unsubmitted() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.get_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"));
    }

    /// #95: enroll's event carries the enrollment timestamp, so indexers
    /// don't have to follow up with a get_progress call to learn it.
    #[test]
    fn test_enroll_event_includes_timestamp() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        env.ledger().with_mut(|l| l.timestamp = 12345);
        client.enroll(&learner, &course_id);

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.enrolled_at, 12345);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            vec![&env, last],
            vec![
                &env,
                (
                    contract_id,
                    (symbol_short!("enrolled"),).into_val(&env),
                    (learner, course_id, 12345u64).into_val(&env),
                )
            ]
        );
    }

    /// #96: credential_eligible fires exactly on the false -> true flip, not
    /// on every write, so indexers don't have to poll get_progress.
    #[test]
    fn test_credential_eligible_event_emitted_on_flip_to_true() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));

        // Not yet eligible (mod_3 and both quizzes still missing) -- the last
        // event so far must be module_completed, not credential_eligible.
        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            vec![&env, last],
            vec![
                &env,
                (
                    contract_id.clone(),
                    (Symbol::new(&env, "module_completed"),).into_val(&env),
                    (
                        learner.clone(),
                        course_id.clone(),
                        Symbol::new(&env, "mod_2"),
                        46u32,
                    )
                        .into_val(&env),
                )
            ]
        );
        let events_before_eligible = all.len();

        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_3"));
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);
        // The submission that completes every requirement must publish
        // credential_eligible as the final event.
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &70);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            vec![&env, last],
            vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "credential_eligible"),).into_val(&env),
                    (learner, course_id).into_val(&env),
                )
            ]
        );
        // module_completed + quiz_submitted x2 + credential_eligible = 4 new events.
        assert_eq!(all.len(), events_before_eligible + 4);
    }

    /// #97: Course carries module_ids (mirroring quiz_ids), so complete_module
    /// only reads one storage key for course configuration instead of two.
    #[test]
    fn test_course_carries_module_ids() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);

        let course = client.get_course(&course_id);
        assert_eq!(course.module_ids.len(), 3);
        assert_eq!(
            course.module_ids.get(0).unwrap(),
            Symbol::new(&env, "mod_1")
        );
        assert_eq!(
            course.module_ids.get(2).unwrap(),
            Symbol::new(&env, "mod_3")
        );

        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        let progress = client.get_progress(&learner, &course_id);
        assert!(progress.overall_progress > 0);
    }

    /// #98: is_eligible_for_credential serves the cached field on ProgressInfo
    /// instead of re-deriving it from Course + ModuleCompleted on every read.
    #[test]
    fn test_is_eligible_for_credential_returns_cached_field() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        assert!(!client.is_eligible_for_credential(&learner, &course_id));

        // Flip the cached field directly, without completing any module or
        // quiz. If the getter recomputed from scratch it would still report
        // false; it must report the cached field instead.
        let mut progress = client.get_progress(&learner, &course_id);
        progress.eligible_for_credential = true;
        env.as_contract(&contract_id, || {
            env.storage().persistent().set(
                &ProgressTrackerDataKey::Progress(learner.clone(), course_id.clone()),
                &progress,
            );
        });

        assert!(client.is_eligible_for_credential(&learner, &course_id));
    }

    // ── Issue #231: course prerequisites ──────────────────────────────────

    /// Register a second, single-module/single-quiz course, used as a
    /// prerequisite in the tests below.
    fn create_prereq_course(env: &Env, client: &ProgressTrackerClient, name: &str) -> Symbol {
        let course_id = Symbol::new(env, name);
        let mut module_ids = Vec::new(env);
        module_ids.push_back(Symbol::new(env, "p_mod_1"));
        let mut quiz_ids = Vec::new(env);
        quiz_ids.push_back(Symbol::new(env, "p_quiz_1"));
        client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
        course_id
    }

    /// Take `learner` all the way to credential eligibility in a course built
    /// by [`create_prereq_course`].
    fn complete_prereq_course(
        client: &ProgressTrackerClient,
        env: &Env,
        learner: &Address,
        course_id: &Symbol,
    ) {
        client.enroll(learner, course_id);
        client.complete_module(learner, course_id, &Symbol::new(env, "p_mod_1"));
        client.submit_quiz_score(learner, course_id, &Symbol::new(env, "p_quiz_1"), &90);
        assert!(client.is_eligible_for_credential(learner, course_id));
    }

    #[test]
    fn test_courses_have_no_prerequisites_by_default() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);

        assert_eq!(client.get_prerequisites(&course_id).len(), 0);
        assert_eq!(client.get_course(&course_id).prerequisites.len(), 0);
    }

    #[test]
    fn test_set_and_get_prerequisites() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let advanced = create_test_course(&env, &client);
        let basics = create_prereq_course(&env, &client, "rust_basics");

        let mut prereqs = Vec::new(&env);
        prereqs.push_back(basics.clone());
        client.set_prerequisites(&advanced, &prereqs);

        assert_eq!(client.get_prerequisites(&advanced), prereqs);
        // Prerequisites are queryable from the Course struct too.
        assert_eq!(client.get_course(&advanced).prerequisites, prereqs);
    }

    #[test]
    fn test_set_prerequisites_emits_event() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let advanced = create_test_course(&env, &client);
        let basics = create_prereq_course(&env, &client, "rust_basics");

        let mut prereqs = Vec::new(&env);
        prereqs.push_back(basics);
        client.set_prerequisites(&advanced, &prereqs);

        let events = env.events().all();
        let (_, topics, _) = events.last().unwrap();
        assert_eq!(
            topics,
            (Symbol::new(&env, "prerequisites_set"),).into_val(&env)
        );
    }

    #[test]
    fn test_set_prerequisites_replaces_previous_list() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let advanced = create_test_course(&env, &client);
        let basics = create_prereq_course(&env, &client, "rust_basics");

        let mut prereqs = Vec::new(&env);
        prereqs.push_back(basics);
        client.set_prerequisites(&advanced, &prereqs);
        assert_eq!(client.get_prerequisites(&advanced).len(), 1);

        // An empty list clears the requirement.
        client.set_prerequisites(&advanced, &Vec::new(&env));
        assert_eq!(client.get_prerequisites(&advanced).len(), 0);
    }

    #[test]
    #[should_panic(expected = "prerequisite not completed")]
    fn test_enroll_rejects_learner_without_prerequisite() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let advanced = create_test_course(&env, &client);
        let basics = create_prereq_course(&env, &client, "rust_basics");

        let mut prereqs = Vec::new(&env);
        prereqs.push_back(basics);
        client.set_prerequisites(&advanced, &prereqs);

        // Never enrolled in the prerequisite at all.
        let learner = Address::generate(&env);
        client.enroll(&learner, &advanced);
    }

    #[test]
    #[should_panic(expected = "prerequisite not completed")]
    fn test_enroll_rejects_learner_with_incomplete_prerequisite() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let advanced = create_test_course(&env, &client);
        let basics = create_prereq_course(&env, &client, "rust_basics");

        let mut prereqs = Vec::new(&env);
        prereqs.push_back(basics.clone());
        client.set_prerequisites(&advanced, &prereqs);

        // Enrolled in the prerequisite but not finished: the module is done
        // and the quiz is not, so eligibility is still false.
        let learner = Address::generate(&env);
        client.enroll(&learner, &basics);
        client.complete_module(&learner, &basics, &Symbol::new(&env, "p_mod_1"));

        client.enroll(&learner, &advanced);
    }

    #[test]
    fn test_enroll_allows_learner_who_completed_prerequisite() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let advanced = create_test_course(&env, &client);
        let basics = create_prereq_course(&env, &client, "rust_basics");

        let mut prereqs = Vec::new(&env);
        prereqs.push_back(basics.clone());
        client.set_prerequisites(&advanced, &prereqs);

        let learner = Address::generate(&env);
        complete_prereq_course(&client, &env, &learner, &basics);

        client.enroll(&learner, &advanced);
        assert_eq!(client.get_progress(&learner, &advanced).overall_progress, 0);
    }

    #[test]
    #[should_panic(expected = "prerequisite not completed")]
    fn test_enroll_requires_every_prerequisite() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let advanced = create_test_course(&env, &client);
        let basics = create_prereq_course(&env, &client, "rust_basics");

        // A second prerequisite the learner never touches.
        let extra = Symbol::new(&env, "rust_extra");
        let mut extra_modules = Vec::new(&env);
        extra_modules.push_back(Symbol::new(&env, "e_mod_1"));
        let mut extra_quizzes = Vec::new(&env);
        extra_quizzes.push_back(Symbol::new(&env, "e_quiz_1"));
        client.create_course(&extra, &1, &1, &extra_modules, &extra_quizzes);

        let mut prereqs = Vec::new(&env);
        prereqs.push_back(basics.clone());
        prereqs.push_back(extra);
        client.set_prerequisites(&advanced, &prereqs);

        let learner = Address::generate(&env);
        complete_prereq_course(&client, &env, &learner, &basics);

        client.enroll(&learner, &advanced);
    }

    #[test]
    fn test_enroll_unaffected_when_course_has_no_prerequisites() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        assert_eq!(
            client.get_progress(&learner, &course_id).overall_progress,
            0
        );
    }

    #[test]
    #[should_panic(expected = "prerequisite course not found")]
    fn test_set_prerequisites_rejects_unknown_course() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let advanced = create_test_course(&env, &client);

        let mut prereqs = Vec::new(&env);
        prereqs.push_back(Symbol::new(&env, "ghost_course"));
        client.set_prerequisites(&advanced, &prereqs);
    }

    #[test]
    #[should_panic(expected = "course cannot be its own prerequisite")]
    fn test_set_prerequisites_rejects_self_reference() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);

        let mut prereqs = Vec::new(&env);
        prereqs.push_back(course_id.clone());
        client.set_prerequisites(&course_id, &prereqs);
    }

    #[test]
    #[should_panic(expected = "duplicate prerequisite found")]
    fn test_set_prerequisites_rejects_duplicates() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let advanced = create_test_course(&env, &client);
        let basics = create_prereq_course(&env, &client, "rust_basics");

        let mut prereqs = Vec::new(&env);
        prereqs.push_back(basics.clone());
        prereqs.push_back(basics);
        client.set_prerequisites(&advanced, &prereqs);
    }

    #[test]
    #[should_panic(expected = "course not found")]
    fn test_set_prerequisites_on_unknown_course_panics() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        client.set_prerequisites(&Symbol::new(&env, "ghost_course"), &Vec::new(&env));
    }

    #[test]
    #[should_panic(expected = "course not found")]
    fn test_get_prerequisites_on_unknown_course_panics() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        client.get_prerequisites(&Symbol::new(&env, "ghost_course"));
    }

    // ── Issue #232: learner statistics aggregation ────────────────────────

    /// Register a second three-module/two-quiz course so aggregates can be
    /// checked across more than one enrollment.
    fn create_second_course(env: &Env, client: &ProgressTrackerClient) -> Symbol {
        let course_id = Symbol::new(env, "rust_202");
        let mut module_ids = Vec::new(env);
        module_ids.push_back(Symbol::new(env, "s_mod_1"));
        module_ids.push_back(Symbol::new(env, "s_mod_2"));
        let mut quiz_ids = Vec::new(env);
        quiz_ids.push_back(Symbol::new(env, "s_quiz_1"));
        client.create_course(&course_id, &2, &1, &module_ids, &quiz_ids);
        course_id
    }

    #[test]
    fn test_learner_stats_for_learner_with_no_enrollments() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let stranger = Address::generate(&env);
        let stats = client.get_learner_stats(&stranger);

        assert_eq!(stats.courses_enrolled, 0);
        assert_eq!(stats.courses_completed, 0);
        assert_eq!(stats.total_quizzes_submitted, 0);
        assert_eq!(stats.total_quiz_score, 0);
        assert_eq!(stats.average_score, 0);
        assert_eq!(stats.total_rewards_earned, 0);
    }

    #[test]
    fn test_learner_stats_counts_enrollments() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let first = create_test_course(&env, &client);
        let second = create_second_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &first);
        client.enroll(&learner, &second);

        let stats = client.get_learner_stats(&learner);
        assert_eq!(stats.courses_enrolled, 2);
        assert_eq!(stats.courses_completed, 0);
    }

    #[test]
    fn test_learner_stats_aggregates_across_courses() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let first = create_test_course(&env, &client);
        let second = create_second_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &first);
        client.submit_quiz_score(&learner, &first, &Symbol::new(&env, "quiz_1"), &80);
        client.submit_quiz_score(&learner, &first, &Symbol::new(&env, "quiz_2"), &90);

        client.enroll(&learner, &second);
        client.submit_quiz_score(&learner, &second, &Symbol::new(&env, "s_quiz_1"), &70);

        let stats = client.get_learner_stats(&learner);
        assert_eq!(stats.courses_enrolled, 2);
        assert_eq!(stats.total_quizzes_submitted, 3);
        assert_eq!(stats.total_quiz_score, 240);
        // (80 + 90 + 70) / 3 = 80
        assert_eq!(stats.average_score, 80);
        // 240 points * BASE_REWARD_PER_POINT (100)
        assert_eq!(stats.total_rewards_earned, 24_000);
    }

    #[test]
    fn test_learner_stats_floors_the_average() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        // (80 + 91) / 2 = 85.5 -> 85
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &91);

        assert_eq!(client.get_learner_stats(&learner).average_score, 85);
    }

    #[test]
    fn test_learner_stats_counts_completed_courses() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let first = create_test_course(&env, &client);
        let second = create_second_course(&env, &client);
        let learner = Address::generate(&env);

        // Finish the first course outright.
        client.enroll(&learner, &first);
        client.complete_module(&learner, &first, &Symbol::new(&env, "mod_1"));
        client.complete_module(&learner, &first, &Symbol::new(&env, "mod_2"));
        client.complete_module(&learner, &first, &Symbol::new(&env, "mod_3"));
        client.submit_quiz_score(&learner, &first, &Symbol::new(&env, "quiz_1"), &80);
        client.submit_quiz_score(&learner, &first, &Symbol::new(&env, "quiz_2"), &80);

        // Only start the second.
        client.enroll(&learner, &second);
        client.complete_module(&learner, &second, &Symbol::new(&env, "s_mod_1"));

        let stats = client.get_learner_stats(&learner);
        assert_eq!(stats.courses_enrolled, 2);
        assert_eq!(stats.courses_completed, 1);
    }

    #[test]
    fn test_learner_stats_is_per_learner() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let strong = Address::generate(&env);
        let weak = Address::generate(&env);

        client.enroll(&strong, &course_id);
        client.enroll(&weak, &course_id);
        client.submit_quiz_score(&strong, &course_id, &Symbol::new(&env, "quiz_1"), &95);
        client.submit_quiz_score(&weak, &course_id, &Symbol::new(&env, "quiz_1"), &55);

        assert_eq!(client.get_learner_stats(&strong).average_score, 95);
        assert_eq!(client.get_learner_stats(&weak).average_score, 55);
        assert_eq!(client.get_learner_stats(&strong).courses_enrolled, 1);
    }

    #[test]
    fn test_learner_stats_ignores_quizless_courses_in_average() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let first = create_test_course(&env, &client);
        let second = create_second_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &first);
        client.submit_quiz_score(&learner, &first, &Symbol::new(&env, "quiz_1"), &60);
        // Enrolled but never submitted a quiz: must not drag the average to 30.
        client.enroll(&learner, &second);

        let stats = client.get_learner_stats(&learner);
        assert_eq!(stats.courses_enrolled, 2);
        assert_eq!(stats.total_quizzes_submitted, 1);
        assert_eq!(stats.average_score, 60);
    }

    #[test]
    fn test_get_learner_courses_lists_enrollments_in_order() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let first = create_test_course(&env, &client);
        let second = create_second_course(&env, &client);
        let learner = Address::generate(&env);

        assert_eq!(client.get_learner_courses(&learner).len(), 0);

        client.enroll(&learner, &first);
        client.enroll(&learner, &second);

        let courses = client.get_learner_courses(&learner);
        assert_eq!(courses.len(), 2);
        assert_eq!(courses.get(0).unwrap(), first);
        assert_eq!(courses.get(1).unwrap(), second);
    }

    // ── Issue #233: lightweight completion percentage query ───────────────

    #[test]
    fn test_completion_percentage_starts_at_zero() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        assert_eq!(client.get_completion_percentage(&learner, &course_id), 0);
    }

    #[test]
    fn test_completion_percentage_matches_stored_progress() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);

        let full = client.get_progress(&learner, &course_id);
        assert_eq!(
            client.get_completion_percentage(&learner, &course_id),
            full.overall_progress
        );
    }

    #[test]
    fn test_completion_percentage_reaches_one_hundred() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_3"));
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &100);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &100);

        assert_eq!(client.get_completion_percentage(&learner, &course_id), 100);
    }

    #[test]
    fn test_completion_percentage_does_not_change_state() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));

        let before = client.get_progress(&learner, &course_id);
        client.get_completion_percentage(&learner, &course_id);
        client.get_completion_percentage(&learner, &course_id);
        let after = client.get_progress(&learner, &course_id);

        assert_eq!(before, after);
    }

    #[test]
    fn test_completion_percentage_is_per_learner_and_course() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let first = create_test_course(&env, &client);
        let second = create_second_course(&env, &client);
        let ahead = Address::generate(&env);
        let behind = Address::generate(&env);

        client.enroll(&ahead, &first);
        client.enroll(&ahead, &second);
        client.enroll(&behind, &first);
        client.complete_module(&ahead, &first, &Symbol::new(&env, "mod_1"));

        assert!(client.get_completion_percentage(&ahead, &first) > 0);
        assert_eq!(client.get_completion_percentage(&ahead, &second), 0);
        assert_eq!(client.get_completion_percentage(&behind, &first), 0);
    }

    #[test]
    #[should_panic(expected = "not enrolled")]
    fn test_completion_percentage_for_unenrolled_learner_panics() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let stranger = Address::generate(&env);

        client.get_completion_percentage(&stranger, &course_id);
    }

    // ── Issue #234: quiz retake support ───────────────────────────────────

    #[test]
    fn test_retake_quiz_replaces_the_score() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_1 = Symbol::new(&env, "quiz_1");

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &quiz_1, &55);
        client.retake_quiz(&learner, &course_id, &quiz_1, &90);

        assert_eq!(client.get_quiz_score(&learner, &course_id, &quiz_1), 90);
    }

    #[test]
    fn test_retake_quiz_updates_the_average() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_1 = Symbol::new(&env, "quiz_1");
        let quiz_2 = Symbol::new(&env, "quiz_2");

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &quiz_1, &50);
        client.submit_quiz_score(&learner, &course_id, &quiz_2, &70);
        assert_eq!(client.get_course_score(&learner, &course_id), 60);

        client.retake_quiz(&learner, &course_id, &quiz_1, &90);

        // (90 + 70) / 2 = 80
        assert_eq!(client.get_course_score(&learner, &course_id), 80);
    }

    #[test]
    fn test_retake_quiz_does_not_double_count_the_quiz() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_1 = Symbol::new(&env, "quiz_1");

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &quiz_1, &40);
        client.retake_quiz(&learner, &course_id, &quiz_1, &80);

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.quizzes_submitted, 1);
        assert_eq!(progress.total_quiz_score, 80);
    }

    #[test]
    fn test_retake_quiz_updates_overall_progress() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_1 = Symbol::new(&env, "quiz_1");

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &quiz_1, &40);
        let before = client.get_completion_percentage(&learner, &course_id);

        client.retake_quiz(&learner, &course_id, &quiz_1, &100);

        assert!(client.get_completion_percentage(&learner, &course_id) > before);
    }

    #[test]
    fn test_retake_quiz_can_unlock_credential_eligibility() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_1 = Symbol::new(&env, "quiz_1");
        let quiz_2 = Symbol::new(&env, "quiz_2");

        client.enroll(&learner, &course_id);
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_3"));
        // Average 40 is below MIN_CREDENTIAL_SCORE (50).
        client.submit_quiz_score(&learner, &course_id, &quiz_1, &40);
        client.submit_quiz_score(&learner, &course_id, &quiz_2, &40);
        assert!(!client.is_eligible_for_credential(&learner, &course_id));

        // Average becomes (100 + 40) / 2 = 70.
        client.retake_quiz(&learner, &course_id, &quiz_1, &100);

        assert!(client.is_eligible_for_credential(&learner, &course_id));
    }

    #[test]
    fn test_retake_quiz_emits_event() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_1 = Symbol::new(&env, "quiz_1");

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &quiz_1, &55);
        client.retake_quiz(&learner, &course_id, &quiz_1, &75);

        let events = env.events().all();
        let (_, topics, _) = events.last().unwrap();
        assert_eq!(topics, (Symbol::new(&env, "quiz_retaken"),).into_val(&env));
    }

    #[test]
    fn test_retake_quiz_updates_the_submission_timestamp() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_1 = Symbol::new(&env, "quiz_1");

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &quiz_1, &55);

        env.ledger().with_mut(|li| li.timestamp = 12_345);
        client.retake_quiz(&learner, &course_id, &quiz_1, &75);

        let export = client.export_progress(&learner, &course_id);
        let result = export.quiz_scores.get(0).unwrap();
        assert_eq!(result.score, 75);
        assert_eq!(result.submitted_at, 12_345);
    }

    #[test]
    fn test_retake_quiz_can_be_repeated_while_improving() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_1 = Symbol::new(&env, "quiz_1");

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &quiz_1, &30);
        client.retake_quiz(&learner, &course_id, &quiz_1, &50);
        client.retake_quiz(&learner, &course_id, &quiz_1, &70);

        assert_eq!(client.get_quiz_score(&learner, &course_id, &quiz_1), 70);
        assert_eq!(
            client.get_progress(&learner, &course_id).total_quiz_score,
            70
        );
    }

    #[test]
    #[should_panic(expected = "new score must be higher")]
    fn test_retake_quiz_rejects_lower_score() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_1 = Symbol::new(&env, "quiz_1");

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &quiz_1, &80);
        client.retake_quiz(&learner, &course_id, &quiz_1, &60);
    }

    #[test]
    #[should_panic(expected = "new score must be higher")]
    fn test_retake_quiz_rejects_equal_score() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_1 = Symbol::new(&env, "quiz_1");

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &quiz_1, &80);
        client.retake_quiz(&learner, &course_id, &quiz_1, &80);
    }

    #[test]
    #[should_panic(expected = "score exceeds maximum")]
    fn test_retake_quiz_rejects_score_above_maximum() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_1 = Symbol::new(&env, "quiz_1");

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &quiz_1, &80);
        client.retake_quiz(&learner, &course_id, &quiz_1, &101);
    }

    #[test]
    #[should_panic(expected = "quiz not submitted")]
    fn test_retake_quiz_rejects_quiz_never_taken() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.retake_quiz(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &90);
    }

    #[test]
    #[should_panic(expected = "not enrolled")]
    fn test_retake_quiz_rejects_unenrolled_learner() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let stranger = Address::generate(&env);

        client.retake_quiz(&stranger, &course_id, &Symbol::new(&env, "quiz_1"), &90);
    }

    #[test]
    fn test_retake_quiz_feeds_learner_stats() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_1 = Symbol::new(&env, "quiz_1");

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &quiz_1, &40);
        client.retake_quiz(&learner, &course_id, &quiz_1, &90);

        let stats = client.get_learner_stats(&learner);
        assert_eq!(stats.total_quizzes_submitted, 1);
        assert_eq!(stats.total_quiz_score, 90);
        assert_eq!(stats.average_score, 90);
        assert_eq!(stats.total_rewards_earned, 9_000);
    }
}
