#![no_std]

mod rewards;
mod types;

use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec};
use types::{Course, DataKey, ProgressInfo, QuizResult};

/// On-chain learning progress tracker for ChainLearn.
///
/// Tracks learner enrollment, module completion, and quiz scores.
/// Provides progress calculation and credential eligibility checks.
#[contract]
pub struct ProgressTracker;

#[contractimpl]
impl ProgressTracker {
    /// Initialize the progress tracker with an admin.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().persistent().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().persistent().set(&DataKey::Admin, &admin);
    }

    /// Register a new course with its modules.
    ///
    /// # Arguments
    /// * `course_id` - Unique course identifier
    /// * `total_modules` - Number of modules in the course
    /// * `total_quizzes` - Number of quizzes in the course
    /// * `module_ids` - List of module identifiers
    pub fn create_course(
        env: Env,
        course_id: Symbol,
        total_modules: u32,
        total_quizzes: u32,
        module_ids: Vec<Symbol>,
    ) {
        let admin: Address = env
            .storage()
            .persistent()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();

        if module_ids.len() != total_modules {
            panic!("module_ids length must match total_modules");
        }

        let course = Course {
            course_id: course_id.clone(),
            total_modules,
            total_quizzes,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Course(course_id.clone()), &course);
        env.storage()
            .persistent()
            .set(&DataKey::CourseModules(course_id.clone()), &module_ids);

        env.events().publish(
            (Symbol::new(&env, "course_created"),),
            (&course_id, total_modules, total_quizzes),
        );
    }

    /// Enroll a learner in a course.
    ///
    /// # Arguments
    /// * `learner` - The learner address (must authorize)
    /// * `course_id` - The course to enroll in
    pub fn enroll(env: Env, learner: Address, course_id: Symbol) {
        learner.require_auth();

        // Verify course exists
        let _course: Course = env
            .storage()
            .persistent()
            .get(&DataKey::Course(course_id.clone()))
            .expect("course not found");

        // Check not already enrolled
        let key = DataKey::Progress(learner.clone(), course_id.clone());
        if env.storage().persistent().has(&key) {
            panic!("already enrolled");
        }

        let progress = ProgressInfo {
            enrolled_at: env.ledger().timestamp(),
            modules_completed: Vec::new(&env),
            quiz_scores: Vec::new(&env),
            overall_progress: 0,
            eligible_for_credential: false,
        };

        env.storage().persistent().set(&key, &progress);

        env.events()
            .publish((Symbol::new(&env, "enrolled"),), (&learner, &course_id));
    }

    /// Mark a module as completed for a learner.
    ///
    /// # Arguments
    /// * `learner` - The learner address (must authorize)
    /// * `course_id` - The course the module belongs to
    /// * `module_id` - The module to mark complete
    pub fn complete_module(env: Env, learner: Address, course_id: Symbol, module_id: Symbol) {
        learner.require_auth();

        // Verify enrollment
        let mut progress: ProgressInfo = env
            .storage()
            .persistent()
            .get(&DataKey::Progress(learner.clone(), course_id.clone()))
            .expect("not enrolled");

        // Check not already completed
        let completed_key =
            DataKey::ModuleCompleted(learner.clone(), course_id.clone(), module_id.clone());
        if env.storage().persistent().has(&completed_key) {
            panic!("module already completed");
        }

        // Verify module exists in course
        let modules: Vec<Symbol> = env
            .storage()
            .persistent()
            .get(&DataKey::CourseModules(course_id.clone()))
            .expect("course modules not found");

        if !modules.contains(&module_id) {
            panic!("module not found in course");
        }

        // Mark module as completed
        env.storage().persistent().set(&completed_key, &true);
        progress.modules_completed.push_back(module_id.clone());

        // Recalculate progress
        let course: Course = env
            .storage()
            .persistent()
            .get(&DataKey::Course(course_id.clone()))
            .expect("course not found");

        progress.overall_progress =
            rewards::calculate_progress(&env, &learner, &course_id, &course);
        progress.eligible_for_credential =
            rewards::is_eligible_for_credential(&env, &learner, &course_id, &course);

        env.storage().persistent().set(
            &DataKey::Progress(learner.clone(), course_id.clone()),
            &progress,
        );

        env.events().publish(
            (Symbol::new(&env, "module_completed"),),
            (&learner, &course_id, &module_id),
        );
    }

    /// Submit a quiz score for a learner.
    ///
    /// # Arguments
    /// * `learner` - The learner address (must authorize)
    /// * `course_id` - The course the quiz belongs to
    /// * `quiz_id` - The quiz identifier
    /// * `score` - The score achieved (0-100)
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
            .get(&DataKey::Progress(learner.clone(), course_id.clone()))
            .expect("not enrolled");

        // Check not already submitted
        let quiz_key = DataKey::QuizResult(learner.clone(), course_id.clone(), quiz_id.clone());
        if env.storage().persistent().has(&quiz_key) {
            panic!("quiz already submitted");
        }

        let result = QuizResult {
            quiz_id: quiz_id.clone(),
            course_id: course_id.clone(),
            score,
            submitted_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&quiz_key, &result);
        progress.quiz_scores.push_back(result);

        // Save progress so recalculations can read updated quiz scores
        env.storage().persistent().set(
            &DataKey::Progress(learner.clone(), course_id.clone()),
            &progress,
        );

        // Recalculate progress
        let course: Course = env
            .storage()
            .persistent()
            .get(&DataKey::Course(course_id.clone()))
            .expect("course not found");

        progress.overall_progress =
            rewards::calculate_progress(&env, &learner, &course_id, &course);
        progress.eligible_for_credential =
            rewards::is_eligible_for_credential(&env, &learner, &course_id, &course);

        env.storage().persistent().set(
            &DataKey::Progress(learner.clone(), course_id.clone()),
            &progress,
        );

        env.events().publish(
            (Symbol::new(&env, "quiz_submitted"),),
            (&learner, &course_id, &quiz_id, score),
        );
    }

    /// Get a learner's progress in a course.
    ///
    /// # Arguments
    /// * `learner` - The learner address
    /// * `course_id` - The course identifier
    ///
    /// # Returns
    /// The `ProgressInfo` for the learner in the given course.
    pub fn get_progress(env: Env, learner: Address, course_id: Symbol) -> ProgressInfo {
        env.storage()
            .persistent()
            .get(&DataKey::Progress(learner, course_id))
            .expect("not enrolled")
    }

    /// Check if a learner is eligible for a credential.
    ///
    /// # Arguments
    /// * `learner` - The learner address
    /// * `course_id` - The course identifier
    pub fn is_eligible_for_credential(env: Env, learner: Address, course_id: Symbol) -> bool {
        let course: Course = env
            .storage()
            .persistent()
            .get(&DataKey::Course(course_id.clone()))
            .expect("course not found");

        rewards::is_eligible_for_credential(&env, &learner, &course_id, &course)
    }

    /// Get course configuration.
    ///
    /// # Arguments
    /// * `course_id` - The course identifier
    pub fn get_course(env: Env, course_id: Symbol) -> Course {
        env.storage()
            .persistent()
            .get(&DataKey::Course(course_id))
            .expect("course not found")
    }

    /// Returns the admin address.
    pub fn admin(env: Env) -> Address {
        env.storage()
            .persistent()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

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
        client.create_course(&course_id, &3, &2, &module_ids);
        course_id
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
        assert_eq!(progress.modules_completed.len(), 0);
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
        assert_eq!(progress.modules_completed.len(), 1);
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
        assert_eq!(progress.quiz_scores.len(), 1);
        assert_eq!(progress.quiz_scores.get(0).unwrap().score, 85);
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
}
