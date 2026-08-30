//! Unit tests for the progress-tracker contract.

use progress_tracker::{ProgressTracker, ProgressTrackerClient};
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    Address, Env, IntoVal, Symbol, Vec,
};

#[cfg(test)]
mod progress_unit_tests {
    use super::*;

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
        assert_eq!(progress.modules_completed_bitmap.count_ones(), 0);
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
        assert_eq!(progress.modules_completed_bitmap.count_ones(), 1);
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
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();

        // Directly insert a course with 0 modules into storage to bypass
        // create_course's own validation, then try to enroll.
        let course_id = Symbol::new(&env, "zero_mod_course");
        let course = progress_tracker::Course {
            course_id: course_id.clone(),
            total_modules: 0,
            total_quizzes: 1,
            module_ids: Vec::new(&env),
            quiz_ids: {
                let mut q = Vec::new(&env);
                q.push_back(Symbol::new(&env, "quiz_1"));
                q
            },
            archived: false,
            content_hash: Symbol::new(&env, "none"),
            prerequisites: Vec::new(&env),
        };
        env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .set(&progress_tracker::ProgressTrackerDataKey::Course(course_id.clone()), &course);
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

    // ── Issue #82 / #113: modules_completed Vec removed (no redundant tracking) ──

    #[test]
    fn test_progress_info_has_no_modules_completed_field() {
        // After removing the redundant modules_completed Vec, ProgressInfo
        // should only have: enrolled_at, quiz_scores, overall_progress,
        // eligible_for_credential. Module completion is tracked solely via
        // the ModuleCompleted storage key -- never duplicated into a Vec (#113).
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
        // A submitted quiz lives in ProgressTrackerDataKey::QuizResult; ProgressInfo keeps
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

    // ── Issue #95: enroll event carries the enrollment timestamp ──────────────

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
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (soroban_sdk::symbol_short!("enrolled"),).into_val(&env),
                    (learner, course_id, 12345u64).into_val(&env),
                )
            ]
        );
    }

    // ── Issue #96: credential_eligible fires exactly on the false→true flip ───

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

        // Not yet eligible (mod_3 and both quizzes are still missing) — the
        // last event so far must be module_completed, not credential_eligible.
        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
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
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
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

    // ── Issue #97: complete_module reads Course once (module_ids included) ────

    #[test]
    fn test_course_carries_module_ids_for_single_read() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);

        // Course itself now carries module_ids (mirroring quiz_ids), so
        // complete_module no longer needs a second storage key just to look
        // up the module list.
        let course = client.get_course(&course_id);
        assert_eq!(course.module_ids.len(), 3);
        assert_eq!(course.module_ids.get(0).unwrap(), Symbol::new(&env, "mod_1"));
        assert_eq!(course.module_ids.get(1).unwrap(), Symbol::new(&env, "mod_2"));
        assert_eq!(course.module_ids.get(2).unwrap(), Symbol::new(&env, "mod_3"));

        // Ordering/ existence checks driven by Course::module_ids still work.
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        let progress = client.get_progress(&learner, &course_id);
        assert!(progress.overall_progress > 0);
    }

    // ── Issue #98: is_eligible_for_credential serves the cached field ─────────

    #[test]
    fn test_is_eligible_for_credential_returns_cached_field() {
        // eligible_for_credential is maintained on every write that could
        // change it, so the public getter should simply return the stored
        // value rather than re-deriving it from Course + ModuleCompleted on
        // every read. Pin that by mutating the stored ProgressInfo directly
        // and confirming the getter reflects it without any further writes.
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        assert!(!client.is_eligible_for_credential(&learner, &course_id));

        let mut progress = client.get_progress(&learner, &course_id);
        progress.eligible_for_credential = true;
        env.as_contract(&contract_id, || {
            env.storage().persistent().set(
                &progress_tracker::ProgressTrackerDataKey::Progress(learner.clone(), course_id.clone()),
                &progress,
            );
        });

        // No module or quiz was actually completed -- if this recomputed from
        // scratch it would still report false. It must report the cached
        // field instead.
        assert!(client.is_eligible_for_credential(&learner, &course_id));
    }

    #[test]
    #[should_panic(expected = "module not found in course")]
    fn test_complete_module_non_existent_module_panics() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let learner = Address::generate(&env);
        let course_id = create_test_course(&env, &client);

        client.enroll(&learner, &course_id);

        let non_existent_mod = Symbol::new(&env, "invalid_mod");
        client.complete_module(&learner, &course_id, &non_existent_mod);
    }

    #[test]
    #[should_panic(expected = "not enrolled")]
    fn test_submit_quiz_score_without_enrollment_panics() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let learner = Address::generate(&env);
        let course_id = create_test_course(&env, &client);

        // Skip enrollment
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &85);
    }

    #[test]
    #[should_panic(expected = "quiz not submitted")]
    fn test_get_quiz_score_mismatched_course_id_panics() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &85);

        let other_course_id = Symbol::new(&env, "other_course");
        let mut module_ids = Vec::new(&env);
        module_ids.push_back(Symbol::new(&env, "mod_1"));
        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(Symbol::new(&env, "quiz_other"));
        client.create_course(&other_course_id, &1, &1, &module_ids, &quiz_ids);

        // quiz_1 is in course_id, not in other_course_id
        client.get_quiz_score(&learner, &other_course_id, &Symbol::new(&env, "quiz_1"));
    }

    #[test]
    fn test_initialize_twice_returns_already_initialized_error() {
        let env = Env::default();
        let (admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let result = client.try_initialize(&admin);

        assert!(result.is_err(), "second initialize call should fail");
        let contract_err = result
            .err()
            .expect("expected an error")
            .expect("expected a typed contract error, not a host trap");
        assert_eq!(
            contract_err,
            progress_tracker::ContractError::AlreadyInitialized
        );
    }
    // ── Issue #233: get_completion_percentage ──────────────────────────────

    #[test]
    fn test_get_completion_percentage_matches_overall_progress() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        assert_eq!(client.get_completion_percentage(&learner, &course_id), 0);

        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(
            client.get_completion_percentage(&learner, &course_id),
            progress.overall_progress
        );
        assert!(progress.overall_progress > 0);
    }

    #[test]
    #[should_panic(expected = "not enrolled")]
    fn test_get_completion_percentage_without_enrollment_panics() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        // Skip enrollment
        client.get_completion_percentage(&learner, &course_id);
    }

    // ── Issue #232: get_learner_courses / get_learner_stats ────────────────

    #[test]
    fn test_get_learner_courses_empty_for_new_learner() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        assert_eq!(client.get_learner_courses(&learner).len(), 0);
    }

    #[test]
    fn test_get_learner_courses_tracks_enrollment_order() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_a = create_test_course(&env, &client);
        let course_b = Symbol::new(&env, "course_b");
        let mut module_ids = Vec::new(&env);
        module_ids.push_back(Symbol::new(&env, "mod_a"));
        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(Symbol::new(&env, "quiz_a"));
        client.create_course(&course_b, &1, &1, &module_ids, &quiz_ids);

        let learner = Address::generate(&env);
        client.enroll(&learner, &course_a);
        client.enroll(&learner, &course_b);

        let courses = client.get_learner_courses(&learner);
        assert_eq!(courses.len(), 2);
        assert_eq!(courses.get(0).unwrap(), course_a);
        assert_eq!(courses.get(1).unwrap(), course_b);
    }

    #[test]
    fn test_get_learner_stats_all_zero_for_new_learner() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        let stats = client.get_learner_stats(&learner);

        assert_eq!(stats.courses_enrolled, 0);
        assert_eq!(stats.courses_completed, 0);
        assert_eq!(stats.total_quizzes_submitted, 0);
        assert_eq!(stats.total_quiz_score, 0);
        assert_eq!(stats.average_score, 0);
        assert_eq!(stats.total_rewards_earned, 0);
    }

    #[test]
    fn test_get_learner_stats_aggregates_across_courses() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let learner = Address::generate(&env);

        // Course A: fully completed and eligible for a credential.
        let course_a = create_test_course(&env, &client);
        client.enroll(&learner, &course_a);
        client.complete_module(&learner, &course_a, &Symbol::new(&env, "mod_1"));
        client.complete_module(&learner, &course_a, &Symbol::new(&env, "mod_2"));
        client.complete_module(&learner, &course_a, &Symbol::new(&env, "mod_3"));
        client.submit_quiz_score(&learner, &course_a, &Symbol::new(&env, "quiz_1"), &80);
        client.submit_quiz_score(&learner, &course_a, &Symbol::new(&env, "quiz_2"), &70);
        assert!(client.get_progress(&learner, &course_a).eligible_for_credential);

        // Course B: enrolled and quizzed, but the module is never completed,
        // so it must not count toward courses_completed.
        let course_b = Symbol::new(&env, "course_b");
        let mut module_ids = Vec::new(&env);
        module_ids.push_back(Symbol::new(&env, "mod_a"));
        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(Symbol::new(&env, "quiz_a"));
        client.create_course(&course_b, &1, &1, &module_ids, &quiz_ids);
        client.enroll(&learner, &course_b);
        client.submit_quiz_score(&learner, &course_b, &Symbol::new(&env, "quiz_a"), &60);
        assert!(!client.get_progress(&learner, &course_b).eligible_for_credential);

        let stats = client.get_learner_stats(&learner);
        assert_eq!(stats.courses_enrolled, 2);
        assert_eq!(stats.courses_completed, 1);
        assert_eq!(stats.total_quizzes_submitted, 3);
        assert_eq!(stats.total_quiz_score, 210);
        assert_eq!(stats.average_score, 70);
        assert_eq!(stats.total_rewards_earned, 21_000);
    }

    #[test]
    fn test_get_learner_stats_average_divides_by_quizzes_not_courses() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let learner = Address::generate(&env);

        // Enrolled in a course but never submits a quiz -- this must not
        // drag the average down by padding the divisor.
        let course_a = create_test_course(&env, &client);
        client.enroll(&learner, &course_a);

        let course_b = Symbol::new(&env, "course_b");
        let mut module_ids = Vec::new(&env);
        module_ids.push_back(Symbol::new(&env, "mod_a"));
        let mut quiz_ids = Vec::new(&env);
        quiz_ids.push_back(Symbol::new(&env, "quiz_a"));
        client.create_course(&course_b, &1, &1, &module_ids, &quiz_ids);
        client.enroll(&learner, &course_b);
        client.submit_quiz_score(&learner, &course_b, &Symbol::new(&env, "quiz_a"), &90);

        let stats = client.get_learner_stats(&learner);
        assert_eq!(stats.courses_enrolled, 2);
        assert_eq!(stats.total_quizzes_submitted, 1);
        assert_eq!(stats.average_score, 90);
    }

    // ── Issue #234: retake_quiz ─────────────────────────────────────────────

    #[test]
    fn test_retake_quiz_replaces_score_without_double_counting() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_id = Symbol::new(&env, "quiz_1");

        client.enroll(&learner, &course_id);
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.submit_quiz_score(&learner, &course_id, &quiz_id, &40);
        client.retake_quiz(&learner, &course_id, &quiz_id, &90);

        assert_eq!(client.get_quiz_score(&learner, &course_id, &quiz_id), 90);

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.quizzes_submitted, 1);
        assert_eq!(progress.total_quiz_score, 90);
    }

    #[test]
    fn test_retake_quiz_emits_quiz_retaken_event() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_id = Symbol::new(&env, "quiz_1");

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &quiz_id, &40);
        client.retake_quiz(&learner, &course_id, &quiz_id, &90);

        // Only mod_1 is uncompleted-out-of-3, so eligibility cannot flip --
        // the last event must be exactly quiz_retaken, not credential_eligible.
        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "quiz_retaken"),).into_val(&env),
                    (learner, course_id, quiz_id, 40u32, 90u32).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_retake_quiz_flips_eligibility_and_emits_credential_eligible() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_id = Symbol::new(&env, "quiz_1");

        client.enroll(&learner, &course_id);
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_2"));
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_3"));
        client.submit_quiz_score(&learner, &course_id, &quiz_id, &20);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2"), &20);
        assert!(!client.get_progress(&learner, &course_id).eligible_for_credential);

        client.retake_quiz(&learner, &course_id, &quiz_id, &90);

        let progress = client.get_progress(&learner, &course_id);
        assert!(progress.eligible_for_credential);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "credential_eligible"),).into_val(&env),
                    (learner, course_id).into_val(&env),
                )
            ]
        );
    }

    #[test]
    #[should_panic(expected = "new score must be higher")]
    fn test_retake_quiz_rejects_non_improving_score() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_id = Symbol::new(&env, "quiz_1");

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &quiz_id, &50);
        client.retake_quiz(&learner, &course_id, &quiz_id, &50);
    }

    #[test]
    #[should_panic(expected = "score exceeds maximum")]
    fn test_retake_quiz_rejects_score_above_max() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let quiz_id = Symbol::new(&env, "quiz_1");

        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &quiz_id, &50);
        client.retake_quiz(&learner, &course_id, &quiz_id, &101);
    }

    #[test]
    #[should_panic(expected = "quiz not submitted")]
    fn test_retake_quiz_without_prior_submission_panics() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        // No submit_quiz_score call -- a first attempt still goes through
        // submit_quiz_score, not retake_quiz.
        client.retake_quiz(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &90);
    }

    #[test]
    #[should_panic(expected = "not enrolled")]
    fn test_retake_quiz_without_enrollment_panics() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        // Skip enrollment
        client.retake_quiz(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &90);
    }

    // ── Issue #211: export_progress ─────────────────────────────────────────

    #[test]
    fn test_export_progress_returns_complete_data() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = 12345);
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.complete_module(&learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &85);

        let export = client.export_progress(&learner, &course_id);
        assert!(export.enrolled);
        assert_eq!(export.enrolled_at, 12345);
        assert_eq!(export.modules_completed_bitmap, 1);
        assert_eq!(export.total_modules, 3);
        assert_eq!(export.quizzes_submitted, 1);
        assert_eq!(export.total_quiz_score, 85);
        assert!(export.overall_progress > 0);
        assert!(!export.eligible_for_credential);
        assert_eq!(export.quiz_scores.len(), 1);
        assert_eq!(export.quiz_scores.get(0).unwrap().score, 85);
    }

    #[test]
    #[should_panic(expected = "not enrolled")]
    fn test_export_progress_not_enrolled_panics() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        client.export_progress(&learner, &course_id);
    }

    // ── Issue #220: batch module completion ──────────────────────────────

    #[test]
    fn test_batch_complete_module_completes_every_module_in_order() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);

        let mut batch = Vec::new(&env);
        batch.push_back(Symbol::new(&env, "mod_1"));
        batch.push_back(Symbol::new(&env, "mod_2"));
        batch.push_back(Symbol::new(&env, "mod_3"));

        client.batch_complete_module(&learner, &course_id, &batch);

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.modules_completed_bitmap.count_ones(), 3);
    }

    #[test]
    fn test_batch_complete_module_matches_sequential_single_calls() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);

        let batch_learner = Address::generate(&env);
        let single_learner = Address::generate(&env);
        client.enroll(&batch_learner, &course_id);
        client.enroll(&single_learner, &course_id);

        let mut batch = Vec::new(&env);
        batch.push_back(Symbol::new(&env, "mod_1"));
        batch.push_back(Symbol::new(&env, "mod_2"));
        batch.push_back(Symbol::new(&env, "mod_3"));
        client.batch_complete_module(&batch_learner, &course_id, &batch);

        client.complete_module(&single_learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.complete_module(&single_learner, &course_id, &Symbol::new(&env, "mod_2"));
        client.complete_module(&single_learner, &course_id, &Symbol::new(&env, "mod_3"));

        let batch_progress = client.get_progress(&batch_learner, &course_id);
        let single_progress = client.get_progress(&single_learner, &course_id);
        assert_eq!(
            batch_progress.modules_completed_bitmap,
            single_progress.modules_completed_bitmap
        );
        assert_eq!(
            batch_progress.overall_progress,
            single_progress.overall_progress
        );
        assert_eq!(
            batch_progress.eligible_for_credential,
            single_progress.eligible_for_credential
        );
    }

    #[test]
    #[should_panic(expected = "module not found in course")]
    fn test_batch_complete_module_aborts_whole_batch_on_invalid_module() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);

        let mut batch = Vec::new(&env);
        batch.push_back(Symbol::new(&env, "mod_1"));
        batch.push_back(Symbol::new(&env, "not_a_real_module"));
        batch.push_back(Symbol::new(&env, "mod_2"));

        // The whole call panics, so this returned Vec should never observe
        // mod_1 or mod_2 as completed. We can't inspect that from inside a
        // should_panic test directly, but a subsequent test verifies the
        // revert by checking storage state after the same panic.
        client.batch_complete_module(&learner, &course_id, &batch);
    }

    #[test]
    fn test_batch_complete_module_reverts_all_on_partial_failure() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);

        let mut batch = Vec::new(&env);
        batch.push_back(Symbol::new(&env, "mod_1"));
        batch.push_back(Symbol::new(&env, "not_a_real_module"));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.batch_complete_module(&learner, &course_id, &batch);
        }));
        assert!(result.is_err());

        // mod_1 must not have been left completed by the aborted batch.
        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.modules_completed_bitmap.count_ones(), 0);
    }

    #[test]
    #[should_panic(expected = "not enrolled")]
    fn test_batch_complete_module_requires_enrollment() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        let mut batch = Vec::new(&env);
        batch.push_back(Symbol::new(&env, "mod_1"));
        client.batch_complete_module(&learner, &course_id, &batch);
    }

    #[test]
    #[should_panic(expected = "module already completed")]
    fn test_batch_complete_module_rejects_duplicate_within_batch() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);

        let mut batch = Vec::new(&env);
        batch.push_back(Symbol::new(&env, "mod_1"));
        batch.push_back(Symbol::new(&env, "mod_1"));

        client.batch_complete_module(&learner, &course_id, &batch);
    }

    #[test]
    fn test_batch_complete_module_emits_module_completed_events() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);

        let mut batch = Vec::new(&env);
        batch.push_back(Symbol::new(&env, "mod_1"));
        batch.push_back(Symbol::new(&env, "mod_2"));

        client.batch_complete_module(&learner, &course_id, &batch);

        let events = env.events().all();
        let module_completed_count = events
            .iter()
            .filter(|(_, topics, _)| {
                let event_name: Symbol = topics.get(0).unwrap().into_val(&env);
                event_name == Symbol::new(&env, "module_completed")
            })
            .count();
        assert_eq!(module_completed_count, 2);
    }

    // ── Issue #221: batch quiz submission ────────────────────────────────

    #[test]
    fn test_batch_submit_quiz_score_submits_every_quiz() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);

        let mut scores = Vec::new(&env);
        scores.push_back((Symbol::new(&env, "quiz_1"), 70u32));
        scores.push_back((Symbol::new(&env, "quiz_2"), 90u32));

        let submitted = client.batch_submit_quiz_score(&learner, &course_id, &scores);

        assert_eq!(submitted.len(), 2);
        assert!(submitted.contains(Symbol::new(&env, "quiz_1")));
        assert!(submitted.contains(Symbol::new(&env, "quiz_2")));

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.quizzes_submitted, 2);
        assert_eq!(progress.total_quiz_score, 160);
        assert_eq!(
            client.get_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1")),
            70
        );
        assert_eq!(
            client.get_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_2")),
            90
        );
    }

    #[test]
    fn test_batch_submit_quiz_score_skips_invalid_entries_independently() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);

        // quiz_1 valid, "not_a_real_quiz" invalid (not in course), quiz_2
        // valid -- the batch must process quiz_1 and quiz_2 despite the bad
        // entry in between, unlike batch_complete_module which would abort.
        let mut scores = Vec::new(&env);
        scores.push_back((Symbol::new(&env, "quiz_1"), 60u32));
        scores.push_back((Symbol::new(&env, "not_a_real_quiz"), 50u32));
        scores.push_back((Symbol::new(&env, "quiz_2"), 80u32));

        let submitted = client.batch_submit_quiz_score(&learner, &course_id, &scores);

        assert_eq!(submitted.len(), 2);
        assert!(submitted.contains(Symbol::new(&env, "quiz_1")));
        assert!(submitted.contains(Symbol::new(&env, "quiz_2")));
        assert!(!submitted.contains(Symbol::new(&env, "not_a_real_quiz")));

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.quizzes_submitted, 2);
        assert_eq!(progress.total_quiz_score, 140);
    }

    #[test]
    fn test_batch_submit_quiz_score_skips_already_submitted_quiz() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);

        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &55);

        let mut scores = Vec::new(&env);
        scores.push_back((Symbol::new(&env, "quiz_1"), 99u32));
        scores.push_back((Symbol::new(&env, "quiz_2"), 85u32));

        let submitted = client.batch_submit_quiz_score(&learner, &course_id, &scores);

        // quiz_1 was already submitted, so the batch skips it without
        // touching its recorded score, and still submits quiz_2.
        assert_eq!(submitted.len(), 1);
        assert!(submitted.contains(Symbol::new(&env, "quiz_2")));
        assert_eq!(
            client.get_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1")),
            55
        );
    }

    #[test]
    fn test_batch_submit_quiz_score_skips_score_above_maximum() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);

        let mut scores = Vec::new(&env);
        scores.push_back((Symbol::new(&env, "quiz_1"), 101u32));
        scores.push_back((Symbol::new(&env, "quiz_2"), 80u32));

        let submitted = client.batch_submit_quiz_score(&learner, &course_id, &scores);

        assert_eq!(submitted.len(), 1);
        assert!(submitted.contains(Symbol::new(&env, "quiz_2")));
    }

    #[test]
    fn test_batch_submit_quiz_score_returns_empty_when_all_entries_invalid() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);

        let mut scores = Vec::new(&env);
        scores.push_back((Symbol::new(&env, "not_real_1"), 50u32));
        scores.push_back((Symbol::new(&env, "not_real_2"), 60u32));

        let submitted = client.batch_submit_quiz_score(&learner, &course_id, &scores);

        assert_eq!(submitted.len(), 0);
        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.quizzes_submitted, 0);
    }

    #[test]
    fn test_batch_submit_quiz_score_matches_sequential_single_calls() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);

        let batch_learner = Address::generate(&env);
        let single_learner = Address::generate(&env);
        client.enroll(&batch_learner, &course_id);
        client.enroll(&single_learner, &course_id);

        let mut scores = Vec::new(&env);
        scores.push_back((Symbol::new(&env, "quiz_1"), 65u32));
        scores.push_back((Symbol::new(&env, "quiz_2"), 95u32));
        client.batch_submit_quiz_score(&batch_learner, &course_id, &scores);

        client.submit_quiz_score(&single_learner, &course_id, &Symbol::new(&env, "quiz_1"), &65);
        client.submit_quiz_score(&single_learner, &course_id, &Symbol::new(&env, "quiz_2"), &95);

        let batch_progress = client.get_progress(&batch_learner, &course_id);
        let single_progress = client.get_progress(&single_learner, &course_id);
        assert_eq!(
            batch_progress.total_quiz_score,
            single_progress.total_quiz_score
        );
        assert_eq!(
            batch_progress.overall_progress,
            single_progress.overall_progress
        );
    }

    #[test]
    #[should_panic(expected = "not enrolled")]
    fn test_batch_submit_quiz_score_requires_enrollment() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);

        let mut scores = Vec::new(&env);
        scores.push_back((Symbol::new(&env, "quiz_1"), 80u32));
        client.batch_submit_quiz_score(&learner, &course_id, &scores);
    }

    #[test]
    fn test_batch_submit_quiz_score_can_unlock_credential_eligibility() {
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
        assert!(!client.get_progress(&learner, &course_id).eligible_for_credential);

        let mut scores = Vec::new(&env);
        scores.push_back((Symbol::new(&env, "quiz_1"), 80u32));
        scores.push_back((Symbol::new(&env, "quiz_2"), 70u32));
        client.batch_submit_quiz_score(&learner, &course_id, &scores);

        assert!(client.get_progress(&learner, &course_id).eligible_for_credential);
    }

    // ── Issue #222: progress delegation ──────────────────────────────────

    #[test]
    fn test_delegated_to_is_none_by_default() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        assert_eq!(client.delegated_to(&learner), None);
    }

    #[test]
    fn test_delegate_progress_sets_delegated_to() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let learner = Address::generate(&env);
        let delegate = Address::generate(&env);

        client.delegate_progress(&learner, &delegate);

        assert_eq!(client.delegated_to(&learner), Some(delegate));
    }

    #[test]
    fn test_delegate_progress_emits_event() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let learner = Address::generate(&env);
        let delegate = Address::generate(&env);

        client.delegate_progress(&learner, &delegate);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "progress_delegated"),).into_val(&env),
                    (learner, delegate).into_val(&env),
                )
            ]
        );
    }

    #[test]
    #[should_panic]
    fn test_delegate_progress_requires_learner_auth() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        let delegate = Address::generate(&env);

        // Nobody authorizes the call -- the learner's own auth is required.
        env.mock_auths(&[]);
        client.delegate_progress(&learner, &delegate);
    }

    #[test]
    fn test_delegate_progress_replaces_previous_delegate() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let learner = Address::generate(&env);
        let first_delegate = Address::generate(&env);
        let second_delegate = Address::generate(&env);

        client.delegate_progress(&learner, &first_delegate);
        client.delegate_progress(&learner, &second_delegate);

        assert_eq!(client.delegated_to(&learner), Some(second_delegate));
    }

    #[test]
    fn test_revoke_delegation_clears_delegated_to() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let learner = Address::generate(&env);
        let delegate = Address::generate(&env);

        client.delegate_progress(&learner, &delegate);
        assert_eq!(client.delegated_to(&learner), Some(delegate));

        client.revoke_delegation(&learner);
        assert_eq!(client.delegated_to(&learner), None);
    }

    #[test]
    fn test_revoke_delegation_emits_event() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let learner = Address::generate(&env);
        let delegate = Address::generate(&env);
        client.delegate_progress(&learner, &delegate);

        client.revoke_delegation(&learner);

        let all = env.events().all();
        let last = all.last().expect("no events emitted");
        assert_eq!(
            soroban_sdk::vec![&env, last],
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (Symbol::new(&env, "delegation_revoked"),).into_val(&env),
                    (learner,).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_revoke_delegation_without_active_delegation_is_a_no_op() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let learner = Address::generate(&env);

        // No delegation was ever set -- revoking must not panic.
        client.revoke_delegation(&learner);
        assert_eq!(client.delegated_to(&learner), None);
    }

    #[test]
    #[should_panic]
    fn test_revoke_delegation_requires_learner_auth() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        let learner = Address::generate(&env);
        env.mock_auths(&[]);
        client.revoke_delegation(&learner);
    }

    #[test]
    fn test_complete_module_for_allows_delegate_to_act() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let delegate = Address::generate(&env);

        client.enroll(&learner, &course_id);
        client.delegate_progress(&learner, &delegate);

        // The delegate authorizes and acts; the learner does not sign this
        // call at all.
        client.complete_module_for(&delegate, &learner, &course_id, &Symbol::new(&env, "mod_1"));

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.modules_completed_bitmap.count_ones(), 1);
    }

    #[test]
    fn test_complete_module_for_allows_learner_to_act_as_their_own_caller() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        client.enroll(&learner, &course_id);

        // No delegation was ever set -- the learner can still call the
        // `_for` entry point as their own caller.
        client.complete_module_for(&learner, &learner, &course_id, &Symbol::new(&env, "mod_1"));

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.modules_completed_bitmap.count_ones(), 1);
    }

    #[test]
    #[should_panic(expected = "caller is not the learner or their delegate")]
    fn test_complete_module_for_rejects_non_delegate_caller() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let stranger = Address::generate(&env);
        client.enroll(&learner, &course_id);

        // stranger authorizes (mock_all_auths lets anyone), but is neither
        // the learner nor a delegate -- must be rejected on the merits.
        client.complete_module_for(&stranger, &learner, &course_id, &Symbol::new(&env, "mod_1"));
    }

    #[test]
    fn test_complete_module_for_rejects_former_delegate_after_revocation() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let delegate = Address::generate(&env);
        client.enroll(&learner, &course_id);

        client.delegate_progress(&learner, &delegate);
        client.revoke_delegation(&learner);

        let result = client.try_complete_module_for(
            &delegate,
            &learner,
            &course_id,
            &Symbol::new(&env, "mod_1"),
        );
        assert!(result.is_err());
    }

    #[test]
    #[should_panic]
    fn test_complete_module_for_requires_caller_auth() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let delegate = Address::generate(&env);
        client.enroll(&learner, &course_id);
        client.delegate_progress(&learner, &delegate);

        // The delegate is a valid delegate, but nobody actually signs this
        // particular call -- auth must still be required, not skipped just
        // because a delegate exists.
        env.mock_auths(&[]);
        client.complete_module_for(&delegate, &learner, &course_id, &Symbol::new(&env, "mod_1"));
    }

    #[test]
    fn test_batch_complete_module_for_allows_delegate() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let delegate = Address::generate(&env);
        client.enroll(&learner, &course_id);
        client.delegate_progress(&learner, &delegate);

        let mut batch = Vec::new(&env);
        batch.push_back(Symbol::new(&env, "mod_1"));
        batch.push_back(Symbol::new(&env, "mod_2"));
        client.batch_complete_module_for(&delegate, &learner, &course_id, &batch);

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.modules_completed_bitmap.count_ones(), 2);
    }

    #[test]
    #[should_panic(expected = "caller is not the learner or their delegate")]
    fn test_batch_complete_module_for_rejects_non_delegate_caller() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let stranger = Address::generate(&env);
        client.enroll(&learner, &course_id);

        let mut batch = Vec::new(&env);
        batch.push_back(Symbol::new(&env, "mod_1"));
        client.batch_complete_module_for(&stranger, &learner, &course_id, &batch);
    }

    #[test]
    fn test_submit_quiz_score_for_allows_delegate_to_act() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let delegate = Address::generate(&env);
        client.enroll(&learner, &course_id);
        client.delegate_progress(&learner, &delegate);

        client.submit_quiz_score_for(&delegate, &learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);

        let progress = client.get_progress(&learner, &course_id);
        assert_eq!(progress.quizzes_submitted, 1);
        assert_eq!(progress.total_quiz_score, 80);
    }

    #[test]
    #[should_panic(expected = "caller is not the learner or their delegate")]
    fn test_submit_quiz_score_for_rejects_non_delegate_caller() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let stranger = Address::generate(&env);
        client.enroll(&learner, &course_id);

        client.submit_quiz_score_for(&stranger, &learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);
    }

    #[test]
    fn test_batch_submit_quiz_score_for_allows_delegate() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let delegate = Address::generate(&env);
        client.enroll(&learner, &course_id);
        client.delegate_progress(&learner, &delegate);

        let mut scores = Vec::new(&env);
        scores.push_back((Symbol::new(&env, "quiz_1"), 80u32));
        scores.push_back((Symbol::new(&env, "quiz_2"), 90u32));

        let submitted = client.batch_submit_quiz_score_for(&delegate, &learner, &course_id, &scores);
        assert_eq!(submitted.len(), 2);
    }

    #[test]
    #[should_panic(expected = "caller is not the learner or their delegate")]
    fn test_batch_submit_quiz_score_for_rejects_non_delegate_caller() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let stranger = Address::generate(&env);
        client.enroll(&learner, &course_id);

        let mut scores = Vec::new(&env);
        scores.push_back((Symbol::new(&env, "quiz_1"), 80u32));
        client.batch_submit_quiz_score_for(&stranger, &learner, &course_id, &scores);
    }

    #[test]
    fn test_retake_quiz_for_allows_delegate_to_act() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let delegate = Address::generate(&env);
        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &40);
        client.delegate_progress(&learner, &delegate);

        client.retake_quiz_for(&delegate, &learner, &course_id, &Symbol::new(&env, "quiz_1"), &90);

        assert_eq!(
            client.get_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1")),
            90
        );
    }

    #[test]
    #[should_panic(expected = "caller is not the learner or their delegate")]
    fn test_retake_quiz_for_rejects_non_delegate_caller() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);
        let learner = Address::generate(&env);
        let stranger = Address::generate(&env);
        client.enroll(&learner, &course_id);
        client.submit_quiz_score(&learner, &course_id, &Symbol::new(&env, "quiz_1"), &40);

        client.retake_quiz_for(&stranger, &learner, &course_id, &Symbol::new(&env, "quiz_1"), &90);
    }

    #[test]
    fn test_delegate_for_matches_learner_direct_call_outcome() {
        let env = Env::default();
        let (_admin, contract_id) = setup_contract(&env);
        let client = ProgressTrackerClient::new(&env, &contract_id);

        env.mock_all_auths();
        let course_id = create_test_course(&env, &client);

        let direct_learner = Address::generate(&env);
        let delegated_learner = Address::generate(&env);
        let delegate = Address::generate(&env);

        client.enroll(&direct_learner, &course_id);
        client.enroll(&delegated_learner, &course_id);
        client.delegate_progress(&delegated_learner, &delegate);

        client.complete_module(&direct_learner, &course_id, &Symbol::new(&env, "mod_1"));
        client.submit_quiz_score(&direct_learner, &course_id, &Symbol::new(&env, "quiz_1"), &80);

        client.complete_module_for(
            &delegate,
            &delegated_learner,
            &course_id,
            &Symbol::new(&env, "mod_1"),
        );
        client.submit_quiz_score_for(
            &delegate,
            &delegated_learner,
            &course_id,
            &Symbol::new(&env, "quiz_1"),
            &80,
        );

        let direct_progress = client.get_progress(&direct_learner, &course_id);
        let delegated_progress = client.get_progress(&delegated_learner, &course_id);
        assert_eq!(
            direct_progress.overall_progress,
            delegated_progress.overall_progress
        );
        assert_eq!(
            direct_progress.total_quiz_score,
            delegated_progress.total_quiz_score
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
        assert_eq!(
            metadata.metadata.name,
            soroban_sdk::String::from_str(&env, "progress-tracker")
        );
    }
}
