//! Unit tests for the progress-tracker contract.

use progress_tracker::{ProgressTracker, ProgressTrackerClient};
use soroban_sdk::{testutils::Address as _, Address, Env, Symbol, Vec};

#[cfg(test)]
mod progress_unit_tests {
    use super::*;

    fn setup_contract(env: &Env) -> (Address, Address) {
        let admin = Address::generate(env);
        let contract_id = env.register(ProgressTracker, ());
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
    fn test_enrollment_creates_zero_progress() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        env.mock_all_auths();

        client.enroll(&learner, &course_id);

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.overall_progress, 0);
        assert!(!progress.eligible_for_credential);
        assert_eq!(progress.modules_completed.len(), 0);
        assert_eq!(progress.quiz_scores.len(), 0);
    }

    #[test]
    fn test_module_completion_updates_progress() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        env.mock_all_auths();

        client.enroll(&learner, &course_id);
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.modules_completed.len(), 1);
        assert!(progress.overall_progress > 0);
    }

    #[test]
    fn test_quiz_submission_updates_progress() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        env.mock_all_auths();

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &85);

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.quiz_scores.len(), 1);
        assert_eq!(progress.quiz_scores.get(0).unwrap().score, 85);
    }

    #[test]
    fn test_eligibility_requires_full_completion() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        env.mock_all_auths();

        client.enroll(&learner, &course_id);

        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_3"));

        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &70);

        let progress = client.get_progress(&learner, &course_id);
        assert!(progress.eligible_for_credential);
        assert_eq!(progress.overall_progress, 100);
    }

    #[test]
    fn test_eligibility_fails_with_low_quiz_scores() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        env.mock_all_auths();

        client.enroll(&learner, &course_id);

        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_3"));

        // Average: (30 + 40) / 2 = 35, below 50 threshold
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &30);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &40);

        let progress = client.get_progress(&learner, &course_id);
        assert!(!progress.eligible_for_credential);
    }

    #[test]
    #[should_panic(expected = "already enrolled")]
    fn test_double_enrollment_rejected() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        env.mock_all_auths();

        client.enroll(&learner, &course_id);
        client.enroll(&learner, &course_id);
    }

    #[test]
    #[should_panic(expected = "module already completed")]
    fn test_double_module_completion_rejected() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        env.mock_all_auths();

        client.enroll(&learner, &course_id);
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
    }

    #[test]
    #[should_panic(expected = "quiz already submitted")]
    fn test_double_quiz_submission_rejected() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        env.mock_all_auths();

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &90);
    }

    #[test]
    #[should_panic(expected = "score exceeds maximum")]
    fn test_quiz_score_above_max_rejected() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        env.mock_all_auths();

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &101);
    }

    #[test]
    #[should_panic(expected = "not enrolled")]
    fn test_get_progress_not_enrolled() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.get_progress(&learner, &course_id);
    }

    #[test]
    fn test_admin_can_create_course() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let course_id = create_test_course(&env, &client);
        let course = client.get_course(&course_id);
        assert_eq!(course.total_modules, 3);
        assert_eq!(course.total_quizzes, 2);
    }

    #[test]
    #[should_panic(expected = "module_ids length must match total_modules")]
    fn test_course_creation_rejects_mismatch() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let course_id = Symbol::new(&env, "rust_101");
        let mut two_ids = Vec::new(&env);
        two_ids.push_back(Symbol::new(&env, "mod_1"));
        two_ids.push_back(Symbol::new(&env, "mod_2"));
        client.create_course(&course_id, &3, &2, &two_ids);
    }
}
