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
        let mut quiz_ids = Vec::new(env);
        quiz_ids.push_back(Symbol::new(env, "quiz_1"));
        quiz_ids.push_back(Symbol::new(env, "quiz_2"));
        client.create_course(&course_id, &3, &2, &module_ids, &quiz_ids);
        course_id
    }

    #[test]
    fn test_enrollment_creates_zero_progress() {
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
        assert_eq!(progress.quizzes_submitted, 0);
        assert_eq!(progress.total_quiz_score, 0);
    }

    #[test]
    fn test_module_completion_updates_progress() {
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
    fn test_quiz_submission_updates_progress() {
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
    }

    #[test]
    fn test_eligibility_requires_full_completion() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);

        // Modules must be completed in order (#81)
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_3"));

        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &70);

        let progress = client.get_progress(&learner, &course_id);
        assert!(progress.eligible_for_credential);
        assert_eq!(progress.overall_progress, 92);
    }

    #[test]
    fn test_eligibility_fails_with_low_quiz_scores() {
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

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.enroll(&learner, &course_id);
    }

    #[test]
    #[should_panic(expected = "module already completed")]
    fn test_double_module_completion_rejected() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

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

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

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

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &101);
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
    fn test_admin_can_create_course() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
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

        env.mock_all_auths();
        let course_id = Symbol::new(&env, "rust_101");
        let mut two_ids = Vec::new(&env);
        two_ids.push_back(Symbol::new(&env, "mod_1"));
        two_ids.push_back(Symbol::new(&env, "mod_2"));
        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(Symbol::new(&env, "quiz_1"));
        quiz_ids.push_back(Symbol::new(&env, "quiz_2"));
        client.create_course(&course_id, &3, &2, &two_ids, &quiz_ids);
    }

    // ── Issue #79: create_course rejects zero modules ─────────────────────────

    #[test]
    #[should_panic(expected = "total_modules must be greater than zero")]
    fn test_create_course_rejects_zero_modules() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = Symbol::new(&env, "empty_course");
        let module_ids = Vec::new(&env);
        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(Symbol::new(&env, "quiz_1"));
        client.create_course(&course_id, &0, &1, &module_ids, &quiz_ids);
    }

    // ── Issue #80: enroll rejects course with no modules ──────────────────────

    #[test]
    #[should_panic(expected = "course has no modules")]
    fn test_enroll_rejects_course_with_no_modules() {
        let env = Env::default();
        let (admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();

        // Directly insert a course with 0 modules into storage to bypass
        // create_course's own validation, then try to enroll.
        let course_id = Symbol::new(&env, "zero_mod_course");
        let course = progress_tracker::Course {
            course_id: course_id.clone(),
            total_modules: 0,
            total_quizzes: 1,
            quiz_ids: {
                let mut q = Vec::new(&env);
                q.push_back(Symbol::new(&env, "quiz_1"));
                q
            },
        };
        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&progress_tracker::DataKey::Course(course_id.clone()), &course);
        });

        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);
    }

    // ── Issue #81: complete_module enforces sequential ordering ────────────────

    #[test]
    #[should_panic(expected = "previous module not completed")]
    fn test_complete_module_rejects_out_of_order() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);

        // Try to complete mod_2 before mod_1 — must fail.
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
    }

    #[test]
    fn test_complete_module_first_module_requires_no_prerequisite() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);

        // First module (mod_1, index 0) has no prerequisite — must succeed.
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));

        let progress = client.get_progress(&learner, &course_id);
        assert!(progress.overall_progress > 0);
    }

    #[test]
    fn test_complete_module_sequential_order_succeeds() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);

        // Complete modules in order — must all succeed.
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_3"));

        let progress = client.get_progress(&learner, &course_id);
        // 3/3 modules * 70 = 70% from modules, 0% from quizzes = 70
        assert_eq!(progress.overall_progress, 70);
    }

    // ── Issue #82: modules_completed Vec removed (no redundant tracking) ──────

    #[test]
    fn test_progress_info_has_no_modules_completed_field() {
        // After removing the redundant modules_completed Vec, ProgressInfo
        // should only have: enrolled_at, quiz_scores, overall_progress,
        // eligible_for_credential. Module completion is tracked solely via
        // the ModuleCompleted storage key.
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));

        let progress = client.get_progress(&learner, &course_id);
        // Module completion is reflected in overall_progress, not a Vec field.
        assert!(progress.overall_progress > 0);
        // The struct no longer has modules_completed — if it did, this
        // wouldn't compile.
    }

    // ── Issue #83: quiz results are stored once, not in two places ────────────

    #[test]
    fn test_quiz_result_stored_only_under_quiz_key() {
        // A submitted quiz lives in DataKey::QuizResult; ProgressInfo keeps
        // only the aggregates derived from it. If the struct still carried a
        // quiz_scores Vec, this wouldn't compile.
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &85);

        // The full result is still retrievable from its own storage key.
        assert_eq!(
            client.get_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1")),
            85
        );

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.quizzes_submitted, 1);
        assert_eq!(progress.total_quiz_score, 85);
    }

    #[test]
    fn test_quiz_aggregates_accumulate_across_submissions() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &60);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &90);

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.quizzes_submitted, 2);
        assert_eq!(progress.total_quiz_score, 150);
        // Average 75 → 75 * 30 / 100 = 22 from quizzes, 0 modules completed.
        assert_eq!(progress.overall_progress, 22);
    }

    // ── Issues #84/#85: read-only getters do not clone their arguments ────────

    #[test]
    fn test_read_only_getters_return_stored_values() {
        // get_progress and get_quiz_score move their arguments into the
        // storage key rather than cloning them. Behaviour is unchanged; this
        // pins it so the clone-free reads stay correct.
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &70);

        assert_eq!(
            client.get_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1")),
            70
        );
        assert_eq!(client.get_progress(&learner, &course_id).quizzes_submitted, 1);
    }

    #[test]
    #[should_panic(expected = "quiz not submitted")]
    fn test_get_quiz_score_rejects_unsubmitted_quiz() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.get_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"));
    }

    // ── Issue #86: is_eligible_for_credential requires enrollment ─────────────

    #[test]
    #[should_panic(expected = "not enrolled")]
    fn test_eligibility_rejects_unenrolled_learner() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        // Never enrolled — must fail before any course data is read.
        client.is_eligible_for_credential(&learner, &course_id);
    }

    #[test]
    fn test_eligibility_for_enrolled_learner() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        // Enrolled but nothing completed yet.
        assert!(!client.is_eligible_for_credential(&learner, &course_id));

        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_3"));
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &70);

        assert!(client.is_eligible_for_credential(&learner, &course_id));
    }
}
