//! Integration tests for batch operations across the ChainLearn contracts.
//!
//! Covers `learn_token::batch_claim_reward` (a true batch entry point) plus
//! batch-style flows built from the progress-tracker's per-module and
//! per-quiz functions: completing several modules and submitting several
//! quiz scores in one learner session. In every case each operation is
//! verified to succeed (or be skipped) independently of the others.

mod fixtures;
use fixtures::setup_chainlearn_env;

use learn_token::LearnTokenClient;
use progress_tracker::ProgressTrackerClient;
use soroban_sdk::{Symbol, Vec};

#[test]
fn test_batch_claim_reward_processes_quizzes_independently() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let token_client = LearnTokenClient::new(env, &setup.token_contract_id);
    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);

    let course_id = Symbol::new(env, "course_batch");
    let quiz1 = Symbol::new(env, "quiz_1");
    let quiz2 = Symbol::new(env, "quiz_2");
    let quiz3 = Symbol::new(env, "quiz_3");

    let mut module_ids = Vec::new(env);
    module_ids.push_back(Symbol::new(env, "mod_1"));
    let mut quiz_ids = Vec::new(env);
    quiz_ids.push_back(quiz1.clone());
    quiz_ids.push_back(quiz2.clone());
    quiz_ids.push_back(quiz3.clone());

    progress_client.create_course(&course_id, &1, &3, &module_ids, &quiz_ids);
    progress_client.enroll(learner, &course_id);
    progress_client.submit_quiz_score(learner, &course_id, &quiz1, &80);
    progress_client.submit_quiz_score(learner, &course_id, &quiz2, &60);
    progress_client.submit_quiz_score(learner, &course_id, &quiz3, &90);

    // Claim quiz2's reward individually up front so the batch call below
    // has to skip an already-claimed quiz alongside two unclaimed ones --
    // proving a partial failure doesn't block the other claims in the batch.
    token_client.claim_reward(learner, &course_id, &quiz2);
    assert_eq!(token_client.balance(learner), 6000);

    let mut batch_ids = Vec::new(env);
    batch_ids.push_back(quiz1.clone());
    batch_ids.push_back(quiz2.clone());
    batch_ids.push_back(quiz3.clone());

    let claimed = token_client.batch_claim_reward(learner, &course_id, &batch_ids);

    // Only the two not-yet-claimed quizzes succeed in the batch.
    assert_eq!(claimed.len(), 2);
    assert!(claimed.contains(quiz1.clone()));
    assert!(claimed.contains(quiz3.clone()));
    assert!(!claimed.contains(quiz2.clone()));

    // 80*100 + 60*100 (individual) + 90*100 = 8000 + 6000 + 9000 = 23000
    assert_eq!(token_client.balance(learner), 23000);
    assert_eq!(token_client.total_supply(), 23000);

    // Reclaiming the same batch again succeeds the call but yields nothing,
    // since every quiz in it is now already claimed.
    let reclaimed = token_client.batch_claim_reward(learner, &course_id, &batch_ids);
    assert_eq!(reclaimed.len(), 0);
    assert_eq!(token_client.balance(learner), 23000);
}

#[test]
fn test_batch_module_completion() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);

    let course_id = Symbol::new(env, "course_modules");
    let mut module_ids = Vec::new(env);
    module_ids.push_back(Symbol::new(env, "mod_1"));
    module_ids.push_back(Symbol::new(env, "mod_2"));
    module_ids.push_back(Symbol::new(env, "mod_3"));
    let mut quiz_ids = Vec::new(env);
    quiz_ids.push_back(Symbol::new(env, "quiz_1"));

    progress_client.create_course(&course_id, &3, &1, &module_ids, &quiz_ids);
    progress_client.enroll(learner, &course_id);

    // Batch-complete every module in the course in a single learner session.
    for module_id in module_ids.iter() {
        progress_client.complete_module(learner, &course_id, &module_id);
    }

    let progress = progress_client.get_progress(learner, &course_id);
    assert_eq!(progress.modules_completed_bitmap.count_ones(), 3);
}

#[test]
fn test_batch_quiz_submission() {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    let learner = &setup.learner;
    env.mock_all_auths();

    let progress_client = ProgressTrackerClient::new(env, &setup.progress_contract_id);

    let course_id = Symbol::new(env, "course_quizzes");
    let mut module_ids = Vec::new(env);
    module_ids.push_back(Symbol::new(env, "mod_1"));
    let mut quiz_ids = Vec::new(env);
    quiz_ids.push_back(Symbol::new(env, "quiz_1"));
    quiz_ids.push_back(Symbol::new(env, "quiz_2"));
    quiz_ids.push_back(Symbol::new(env, "quiz_3"));

    progress_client.create_course(&course_id, &1, &3, &module_ids, &quiz_ids);
    progress_client.enroll(learner, &course_id);

    let scores = [70u32, 85u32, 95u32];

    // Batch-submit every quiz score for the course in a single learner
    // session; each submission succeeds independently of the others.
    for (i, quiz_id) in quiz_ids.iter().enumerate() {
        progress_client.submit_quiz_score(learner, &course_id, &quiz_id, &scores[i]);
    }

    for (i, quiz_id) in quiz_ids.iter().enumerate() {
        assert_eq!(
            progress_client.get_quiz_score(learner, &course_id, &quiz_id),
            scores[i]
        );
    }

    let progress = progress_client.get_progress(learner, &course_id);
    assert_eq!(progress.quizzes_submitted, 3);
    assert_eq!(progress.total_quiz_score, 70 + 85 + 95);
}
