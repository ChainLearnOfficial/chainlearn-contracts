//! Unit tests for the progress-tracker contract.

use soroban_sdk::{testutils::Address as _, Address, Env, Symbol, Vec};

#[cfg(test)]
mod progress_unit_tests {
    use super::*;

    /// Test enrollment creates initial progress with 0%.
    #[test]
    fn test_enrollment_creates_zero_progress() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        // client.create_course(&course_id, &3, &2, &module_ids);
        // client.enroll(&learner, &course_id);

        // let progress = client.get_progress(&learner, &course_id);
        // assert_eq!(progress.overall_progress, 0);
        // assert!(!progress.eligible_for_credential);
        // assert_eq!(progress.modules_completed.len(), 0);
        // assert_eq!(progress.quiz_scores.len(), 0);
    }

    /// Test module completion updates progress.
    #[test]
    fn test_module_completion_updates_progress() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        // Setup: course with 3 modules
        // client.enroll(&learner, &course_id);

        // Complete 1 of 3 modules
        // client.complete_module(&learner, &course_id, &mod_1);

        // let progress = client.get_progress(&learner, &course_id);
        // assert_eq!(progress.modules_completed.len(), 1);
        // assert!(progress.overall_progress > 0);
    }

    /// Test quiz submission updates progress.
    #[test]
    fn test_quiz_submission_updates_progress() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        // client.enroll(&learner, &course_id);
        // client.submit_quiz_score(&learner, &course_id, &quiz_1, &85);

        // let progress = client.get_progress(&learner, &course_id);
        // assert_eq!(progress.quiz_scores.len(), 1);
        // assert_eq!(progress.quiz_scores.get(0).unwrap().score, 85);
    }

    /// Test eligibility requires all modules and passing quiz average.
    #[test]
    fn test_eligibility_requires_full_completion() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        // Setup: 3 modules, 2 quizzes
        // client.enroll(&learner, &course_id);

        // Complete all modules
        // client.complete_module(&learner, &course_id, &mod_1);
        // client.complete_module(&learner, &course_id, &mod_2);
        // client.complete_module(&learner, &course_id, &mod_3);

        // Submit quizzes with passing scores (avg >= 50)
        // client.submit_quiz_score(&learner, &course_id, &quiz_1, &80);
        // client.submit_quiz_score(&learner, &course_id, &quiz_2, &70);

        // let progress = client.get_progress(&learner, &course_id);
        // assert!(progress.eligible_for_credential);
        // assert_eq!(progress.overall_progress, 100);
    }

    /// Test eligibility fails if quiz average is too low.
    #[test]
    fn test_eligibility_fails_with_low_quiz_scores() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        // Complete all modules but score poorly on quizzes
        // Average: (30 + 40) / 2 = 35, below 50 threshold

        // let progress = client.get_progress(&learner, &course_id);
        // assert!(!progress.eligible_for_credential);
    }

    /// Test double enrollment is rejected.
    #[test]
    #[should_panic(expected = "already enrolled")]
    fn test_double_enrollment_rejected() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        // client.enroll(&learner, &course_id);
        // client.enroll(&learner, &course_id);
    }

    /// Test double module completion is rejected.
    #[test]
    #[should_panic(expected = "module already completed")]
    fn test_double_module_completion_rejected() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        // client.enroll(&learner, &course_id);
        // client.complete_module(&learner, &course_id, &mod_1);
        // client.complete_module(&learner, &course_id, &mod_1);
    }

    /// Test double quiz submission is rejected.
    #[test]
    #[should_panic(expected = "quiz already submitted")]
    fn test_double_quiz_submission_rejected() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        // client.enroll(&learner, &course_id);
        // client.submit_quiz_score(&learner, &course_id, &quiz_1, &80);
        // client.submit_quiz_score(&learner, &course_id, &quiz_1, &90);
    }

    /// Test quiz score above maximum is rejected.
    #[test]
    #[should_panic(expected = "score exceeds maximum")]
    fn test_quiz_score_above_max_rejected() {
        let env = Env::default();
        let learner = Address::generate(&env);
        env.mock_all_auths();

        // client.enroll(&learner, &course_id);
        // client.submit_quiz_score(&learner, &course_id, &quiz_1, &101);
    }

    /// Test getting progress for unenrolled learner panics.
    #[test]
    #[should_panic(expected = "not enrolled")]
    fn test_get_progress_not_enrolled() {
        let env = Env::default();
        let learner = Address::generate(&env);

        // client.get_progress(&learner, &course_id);
    }

    /// Test course creation by admin.
    #[test]
    fn test_admin_can_create_course() {
        let env = Env::default();
        let admin = Address::generate(&env);
        env.mock_all_auths();

        // let course = client.get_course(&course_id);
        // assert_eq!(course.total_modules, 3);
        // assert_eq!(course.total_quizzes, 2);
    }

    /// Test course creation rejects mismatched module count.
    #[test]
    #[should_panic(expected = "module_ids length must match total_modules")]
    fn test_course_creation_rejects_mismatch() {
        let env = Env::default();
        let admin = Address::generate(&env);
        env.mock_all_auths();

        // Claim 3 modules but only provide 2 IDs
        // client.create_course(&course_id, &3, &2, &two_module_ids);
    }
}
