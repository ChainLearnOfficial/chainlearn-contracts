#![no_std]

mod rewards;
mod types;

use soroban_sdk::{contract, contracterror, contractimpl, symbol_short, Address, Env, Symbol, Vec};
use types::{Course, ProgressInfo, ProgressTrackerDataKey, QuizResult};

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
        Ok(())
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
        learner.require_auth();

        // Verify course exists
        let course: Course = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Course(course_id.clone()))
            .expect("course not found");

        // Verify course has at least one module (#80)
        if course.total_modules == 0 {
            panic!("course has no modules");
        }

        // Check not already enrolled
        let key = ProgressTrackerDataKey::Progress(learner.clone(), course_id.clone());
        if env.storage().persistent().has(&key) {
            panic!("already enrolled");
        }

        let progress = ProgressInfo {
            enrolled_at: env.ledger().timestamp(),
            quizzes_submitted: 0,
            total_quiz_score: 0,
            overall_progress: 0,
            eligible_for_credential: false,
        };

        env.storage().persistent().set(&key, &progress);

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

        // Check not already completed
        let completed_key = ProgressTrackerDataKey::ModuleCompleted(
            learner.clone(),
            course_id.clone(),
            module_id.clone(),
        );
        if env.storage().persistent().has(&completed_key) {
            panic!("module already completed");
        }

        // Single read serves both the module lookup/ordering check below and
        // the progress/eligibility recalculation further down -- Course
        // carries module_ids, so there is no second fetch under a separate
        // key for the same course configuration (#97).
        let course: Course = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::Course(course_id.clone()))
            .expect("course not found");

        if !course.module_ids.contains(&module_id) {
            panic!("module not found in course");
        }

        // Enforce sequential ordering: module at index N requires module N-1 completed
        let mut module_index: Option<u32> = None;
        for (i, m) in course.module_ids.iter().enumerate() {
            if m == module_id {
                module_index = Some(i as u32);
                break;
            }
        }
        if let Some(idx) = module_index {
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
        }

        // Mark module as completed
        env.storage().persistent().set(&completed_key, &true);

        let was_eligible = progress.eligible_for_credential;

        progress.overall_progress =
            rewards::calculate_progress(&env, &learner, &course_id, &course, &progress);
        progress.eligible_for_credential =
            rewards::is_eligible_for_credential(&env, &learner, &course_id, &course, &progress);

        env.storage().persistent().set(
            &ProgressTrackerDataKey::Progress(learner.clone(), course_id.clone()),
            &progress,
        );

        env.events().publish(
            (Symbol::new(&env, "module_completed"),),
            (&learner, &course_id, &module_id),
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
        progress.overall_progress =
            rewards::calculate_progress(&env, &learner, &course_id, &course, &progress);
        progress.eligible_for_credential =
            rewards::is_eligible_for_credential(&env, &learner, &course_id, &course, &progress);

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
        // Read-only: move the arguments into the key instead of cloning them.
        let result: QuizResult = env
            .storage()
            .persistent()
            .get(&ProgressTrackerDataKey::QuizResult(
                learner, course_id, quiz_id,
            ))
            .expect("quiz not submitted");
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
            .get(&DataKey::Progress(learner, course_id))
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
}
