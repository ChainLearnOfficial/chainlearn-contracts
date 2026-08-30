//! Gas benchmark for cross-contract call optimization (#217).
//!
//! Measures host CPU-instruction cost (via `env.budget()`) for the reward flow
//! that hops into the progress-tracker contract, and asserts the batched entry
//! point — which resolves the tracker address and the `get_quiz_score` symbol
//! once and reuses the running supply/balance totals — costs materially less
//! than issuing the same claims one at a time. A regression that reintroduces a
//! per-iteration `Symbol::new`, re-reads the tracker address per quiz, or routes
//! the hot path back through a generated client constructor pushes the ratio
//! back up and fails the test.
//!
//! Native CPU numbers underestimate the WASM equivalent (per the soroban-sdk
//! docs), so the assertions here are ratios between two paths measured the same
//! way, never absolute instruction counts.

#[path = "../integration/fixtures.rs"]
mod fixtures;
use fixtures::setup_chainlearn_env;

use learn_token::LearnTokenClient;
use progress_tracker::ProgressTrackerClient;
use soroban_sdk::{Address, Env, Symbol, Vec};

/// Register a course with `n` quizzes, enroll `learner`, and submit a passing
/// score for every quiz. Returns the course id and the quiz ids.
fn seed_course(
    env: &Env,
    progress: &ProgressTrackerClient,
    learner: &Address,
    n: u32,
) -> (Symbol, Vec<Symbol>) {
    const IDS: [&str; 20] = [
        "q0", "q1", "q2", "q3", "q4", "q5", "q6", "q7", "q8", "q9", "q10", "q11", "q12", "q13",
        "q14", "q15", "q16", "q17", "q18", "q19",
    ];
    assert!(n as usize <= IDS.len(), "extend IDS for n > {}", IDS.len());

    let course_id = Symbol::new(env, "bench_course");
    let mut module_ids = Vec::new(env);
    module_ids.push_back(Symbol::new(env, "mod_1"));

    let mut quiz_ids = Vec::new(env);
    for id in IDS.iter().take(n as usize) {
        quiz_ids.push_back(Symbol::new(env, id));
    }

    progress.create_course(&course_id, &1, &n, &module_ids, &quiz_ids);
    progress.enroll(learner, &course_id);
    for quiz_id in quiz_ids.iter() {
        progress.submit_quiz_score(learner, &course_id, &quiz_id, &80);
    }
    (course_id, quiz_ids)
}

/// CPU cost of claiming `n` quizzes one `claim_reward` call at a time.
fn individual_cpu(n: u32) -> u64 {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    env.mock_all_auths();
    let token = LearnTokenClient::new(env, &setup.token_contract_id);
    let progress = ProgressTrackerClient::new(env, &setup.progress_contract_id);
    let (course_id, quiz_ids) = seed_course(env, &progress, &setup.learner, n);

    env.budget().reset_default();
    for quiz_id in quiz_ids.iter() {
        token.claim_reward(&setup.learner, &course_id, &quiz_id);
    }
    env.budget().cpu_instruction_cost()
}

/// CPU cost of claiming `n` quizzes in a single `batch_claim_reward` call.
fn batch_cpu(n: u32) -> u64 {
    let setup = setup_chainlearn_env();
    let env = &setup.env;
    env.mock_all_auths();
    let token = LearnTokenClient::new(env, &setup.token_contract_id);
    let progress = ProgressTrackerClient::new(env, &setup.progress_contract_id);
    let (course_id, quiz_ids) = seed_course(env, &progress, &setup.learner, n);

    env.budget().reset_default();
    let claimed = token.batch_claim_reward(&setup.learner, &course_id, &quiz_ids);
    assert_eq!(claimed.len(), n);
    env.budget().cpu_instruction_cost()
}

/// One `batch_claim_reward` over N quizzes must cost less host CPU than N
/// separate `claim_reward` calls for the same quizzes, at every batch size.
#[test]
fn batch_claim_beats_individual_claims() {
    for n in [3_u32, 10, 16] {
        let individual = individual_cpu(n);
        let batched = batch_cpu(n);
        let pct = (batched as f64 / individual as f64) * 100.0;

        println!(
            "xcontract bench: {n:>2} claims — individual = {individual:>9} CPU insns, \
             batched = {batched:>9} CPU insns ({pct:.1}% of individual)"
        );

        assert!(
            batched < individual,
            "batch_claim_reward ({batched}) should cost less than {n} individual \
             claim_reward calls ({individual})"
        );
    }
}
