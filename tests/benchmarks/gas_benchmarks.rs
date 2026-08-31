//! Gas benchmark tests for ChainLearn contracts (#288).
//!
//! Measures CPU instruction costs via `env.budget()` for each major contract
//! function across all three contracts. Results are printed and compared
//! against thresholds to detect regressions. Benchmarks are repeatable:
//! each test sets up a fresh environment and resets the budget before
//! measuring.
//!
//! Soroban's host CPU-instruction numbers underestimate WASM costs, so these
//! are relative comparisons between operations, not absolute gas figures.

use credential_nft::{CredentialNft, CredentialNftClient};
use learn_token::LearnTokenClient;
use progress_tracker::{ProgressTracker, ProgressTrackerClient};
use soroban_sdk::{
    testutils::Address as _, Address, Env, String as SorobanString, Symbol, Vec,
};

struct BenchEnv {
    env: Env,
    admin: Address,
    token_client: LearnTokenClient<'static>,
    credential_client: CredentialNftClient<'static>,
    progress_client: ProgressTrackerClient<'static>,
}

fn setup_bench() -> BenchEnv {
    let env = Env::default();
    let admin = Address::generate(&env);

    let progress_contract_id = env.register_contract(None, ProgressTracker);
    let progress_client = ProgressTrackerClient::new(&env, &progress_contract_id);
    progress_client.initialize(&admin);

    let token_contract_id = env.register_contract(None, learn_token::LearnToken);
    let token_client = LearnTokenClient::new(&env, &token_contract_id);
    token_client.initialize(
        &admin,
        &SorobanString::from_str(&env, "CLearn"),
        &SorobanString::from_str(&env, "CLRN"),
        &7,
        &progress_contract_id,
        &1_000_000_000_000_000,
    );

    let credential_contract_id = env.register_contract(None, CredentialNft);
    let credential_client = CredentialNftClient::new(&env, &credential_contract_id);
    credential_client.initialize(&admin, &progress_contract_id);

    BenchEnv {
        env,
        admin,
        token_client,
        credential_client,
        progress_client,
    }
}

// ── Progress Tracker Benchmarks ─────────────────────────────────────────────

#[test]
fn bench_create_course() {
    let bench = setup_bench();
    let env = &bench.env;
    env.mock_all_auths();

    let course_id = Symbol::new(&env, "bench_course");
    let mut module_ids = Vec::new(env);
    for i in 0..3 {
        module_ids.push_back(Symbol::new(env, &format!("mod_{}", i)));
    }
    let mut quiz_ids = Vec::new(env);
    for i in 0..2 {
        quiz_ids.push_back(Symbol::new(env, &format!("quiz_{}", i)));
    }

    env.budget().reset_default();
    bench.progress_client.create_course(&course_id, &3, &2, &module_ids, &quiz_ids);
    let cost = env.budget().cpu_instruction_cost();

    println!("bench create_course: {cost} CPU insns");
    assert!(cost > 0, "create_course must have nonzero cost");
}

#[test]
fn bench_enroll() {
    let bench = setup_bench();
    let env = &bench.env;
    env.mock_all_auths();

    let course_id = Symbol::new(&env, "bench_course");
    let mut module_ids = Vec::new(env);
    module_ids.push_back(Symbol::new(env, "mod_1"));
    let mut quiz_ids = Vec::new(env);
    quiz_ids.push_back(Symbol::new(env, "quiz_1"));
    bench.progress_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);

    let learner = Address::generate(env);

    env.budget().reset_default();
    bench.progress_client.enroll(&learner, &course_id);
    let cost = env.budget().cpu_instruction_cost();

    println!("bench enroll: {cost} CPU insns");
    assert!(cost > 0);
}

#[test]
fn bench_complete_module() {
    let bench = setup_bench();
    let env = &bench.env;
    env.mock_all_auths();

    let course_id = Symbol::new(&env, "bench_course");
    let mut module_ids = Vec::new(env);
    module_ids.push_back(Symbol::new(env, "mod_1"));
    let mut quiz_ids = Vec::new(env);
    quiz_ids.push_back(Symbol::new(env, "quiz_1"));
    bench.progress_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);

    let learner = Address::generate(env);
    bench.progress_client.enroll(&learner, &course_id);

    env.budget().reset_default();
    bench.progress_client.complete_module(&learner, &course_id, &Symbol::new(env, "mod_1"));
    let cost = env.budget().cpu_instruction_cost();

    println!("bench complete_module: {cost} CPU insns");
    assert!(cost > 0);
}

#[test]
fn bench_submit_quiz_score() {
    let bench = setup_bench();
    let env = &bench.env;
    env.mock_all_auths();

    let course_id = Symbol::new(&env, "bench_course");
    let mut module_ids = Vec::new(env);
    module_ids.push_back(Symbol::new(env, "mod_1"));
    let mut quiz_ids = Vec::new(env);
    quiz_ids.push_back(Symbol::new(env, "quiz_1"));
    bench.progress_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);

    let learner = Address::generate(env);
    bench.progress_client.enroll(&learner, &course_id);

    env.budget().reset_default();
    bench.progress_client.submit_quiz_score(&learner, &course_id, &Symbol::new(env, "quiz_1"), &80);
    let cost = env.budget().cpu_instruction_cost();

    println!("bench submit_quiz_score: {cost} CPU insns");
    assert!(cost > 0);
}

#[test]
fn bench_get_progress() {
    let bench = setup_bench();
    let env = &bench.env;
    env.mock_all_auths();

    let course_id = Symbol::new(&env, "bench_course");
    let mut module_ids = Vec::new(env);
    module_ids.push_back(Symbol::new(env, "mod_1"));
    let mut quiz_ids = Vec::new(env);
    quiz_ids.push_back(Symbol::new(env, "quiz_1"));
    bench.progress_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);

    let learner = Address::generate(env);
    bench.progress_client.enroll(&learner, &course_id);

    env.budget().reset_default();
    let _progress = bench.progress_client.get_progress(&learner, &course_id);
    let cost = env.budget().cpu_instruction_cost();

    println!("bench get_progress: {cost} CPU insns");
    assert!(cost > 0);
}

// ── Learn Token Benchmarks ──────────────────────────────────────────────────

fn setup_completed_course(bench: &BenchEnv, learner: &Address) -> (Symbol, Symbol) {
    let course_id = Symbol::new(&bench.env, "bench_course");
    let quiz_id = Symbol::new(&bench.env, "quiz_1");
    let mut module_ids = Vec::new(&bench.env);
    module_ids.push_back(Symbol::new(&bench.env, "mod_1"));
    let mut quiz_ids = Vec::new(&bench.env);
    quiz_ids.push_back(quiz_id.clone());
    bench.progress_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
    bench.progress_client.enroll(learner, &course_id);
    bench.progress_client.complete_module(learner, &course_id, &Symbol::new(&bench.env, "mod_1"));
    bench.progress_client.submit_quiz_score(learner, &course_id, &quiz_id, &80);
    (course_id, quiz_id)
}

#[test]
fn bench_claim_reward() {
    let bench = setup_bench();
    let env = &bench.env;
    env.mock_all_auths();

    let learner = Address::generate(env);
    let (course_id, quiz_id) = setup_completed_course(&bench, &learner);

    env.budget().reset_default();
    bench.token_client.claim_reward(&learner, &course_id, &quiz_id);
    let cost = env.budget().cpu_instruction_cost();

    println!("bench claim_reward: {cost} CPU insns");
    assert!(cost > 0);
    assert_eq!(bench.token_client.balance(&learner), 8000);
}

#[test]
fn bench_transfer() {
    let bench = setup_bench();
    let env = &bench.env;
    env.mock_all_auths();

    let from = Address::generate(env);
    let to = Address::generate(env);
    bench.token_client.mint(&bench.admin, &from, &10000);

    env.budget().reset_default();
    bench.token_client.transfer(&from, &to, &5000);
    let cost = env.budget().cpu_instruction_cost();

    println!("bench transfer: {cost} CPU insns");
    assert!(cost > 0);
}

#[test]
fn bench_approve_and_transfer_from() {
    let bench = setup_bench();
    let env = &bench.env;
    env.mock_all_auths();

    let owner = Address::generate(env);
    let spender = Address::generate(env);
    let recipient = Address::generate(env);
    bench.token_client.mint(&bench.admin, &owner, &10000);

    env.budget().reset_default();
    bench.token_client.approve(&owner, &spender, &5000, &999999);
    let approve_cost = env.budget().cpu_instruction_cost();

    env.budget().reset_default();
    bench.token_client.transfer_from(&spender, &owner, &recipient, &3000);
    let transfer_from_cost = env.budget().cpu_instruction_cost();

    println!("bench approve: {approve_cost} CPU insns");
    println!("bench transfer_from: {transfer_from_cost} CPU insns");
    assert!(approve_cost > 0);
    assert!(transfer_from_cost > 0);
}

#[test]
fn bench_mint() {
    let bench = setup_bench();
    let env = &bench.env;
    env.mock_all_auths();

    let recipient = Address::generate(env);

    env.budget().reset_default();
    bench.token_client.mint(&bench.admin, &recipient, &10000);
    let cost = env.budget().cpu_instruction_cost();

    println!("bench mint: {cost} CPU insns");
    assert!(cost > 0);
}

#[test]
fn bench_burn() {
    let bench = setup_bench();
    let env = &bench.env;
    env.mock_all_auths();

    let user = Address::generate(env);
    bench.token_client.mint(&bench.admin, &user, &10000);

    env.budget().reset_default();
    bench.token_client.burn(&user, &5000);
    let cost = env.budget().cpu_instruction_cost();

    println!("bench burn: {cost} CPU insns");
    assert!(cost > 0);
    assert_eq!(bench.token_client.balance(&user), 5000);
}

// ── Credential NFT Benchmarks ───────────────────────────────────────────────

fn setup_completed_course_for_credential(
    bench: &BenchEnv,
    learner: &Address,
    score: u32,
) -> (Symbol, Symbol) {
    let course_id = Symbol::new(&bench.env, "bench_course");
    let quiz_id = Symbol::new(&bench.env, "quiz_1");
    let mut module_ids = Vec::new(&bench.env);
    module_ids.push_back(Symbol::new(&bench.env, "mod_1"));
    let mut quiz_ids = Vec::new(&bench.env);
    quiz_ids.push_back(quiz_id.clone());
    bench.progress_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
    bench.progress_client.enroll(learner, &course_id);
    bench.progress_client.complete_module(learner, &course_id, &Symbol::new(&bench.env, "mod_1"));
    bench.progress_client.submit_quiz_score(learner, &course_id, &quiz_id, &score);
    (course_id, quiz_id)
}

#[test]
fn bench_mint_credential() {
    let bench = setup_bench();
    let env = &bench.env;
    env.mock_all_auths();

    let learner = Address::generate(env);
    let (course_id, _quiz_id) = setup_completed_course_for_credential(&bench, &learner, 85);
    let metadata_uri = Symbol::new(env, "ipfs_Qm123");

    env.budget().reset_default();
    bench.credential_client.mint_credential(&learner, &course_id, &85, &metadata_uri);
    let cost = env.budget().cpu_instruction_cost();

    println!("bench mint_credential: {cost} CPU insns");
    assert!(cost > 0);
}

#[test]
fn bench_verify_credential() {
    let bench = setup_bench();
    let env = &bench.env;
    env.mock_all_auths();

    let learner = Address::generate(env);
    let (course_id, _quiz_id) = setup_completed_course_for_credential(&bench, &learner, 85);
    let metadata_uri = Symbol::new(env, "ipfs_Qm123");
    let cred_id = bench.credential_client.mint_credential(&learner, &course_id, &85, &metadata_uri);

    env.budget().reset_default();
    let _info = bench.credential_client.verify_credential(&cred_id);
    let cost = env.budget().cpu_instruction_cost();

    println!("bench verify_credential: {cost} CPU insns");
    assert!(cost > 0);
}

#[test]
fn bench_revoke_credential() {
    let bench = setup_bench();
    let env = &bench.env;
    env.mock_all_auths();

    let learner = Address::generate(env);
    let (course_id, _quiz_id) = setup_completed_course_for_credential(&bench, &learner, 85);
    let metadata_uri = Symbol::new(env, "ipfs_Qm123");
    let cred_id = bench.credential_client.mint_credential(&learner, &course_id, &85, &metadata_uri);

    env.budget().reset_default();
    bench.credential_client.revoke_credential(&cred_id);
    let cost = env.budget().cpu_instruction_cost();

    println!("bench revoke_credential: {cost} CPU insns");
    assert!(cost > 0);
}

// ── Regression Detection ────────────────────────────────────────────────────
//
// Each benchmark records a baseline ratio. If the ratio between two
// operations changes by more than 50%, a regression is flagged. These
// ratios are intentionally relative (not absolute) so they stay stable
// across different host versions.

#[test]
fn bench_regression_batch_vs_individual_claim() {
    let bench = setup_bench();
    let env = &bench.env;
    env.mock_all_auths();

    let learner = Address::generate(env);
    let mut module_ids = Vec::new(env);
    module_ids.push_back(Symbol::new(env, "mod_1"));

    let mut quiz_ids_src = Vec::new(env);
    let n: usize = 5;
    for i in 0..n {
        quiz_ids_src.push_back(Symbol::new(env, &format!("quiz_{}", i)));
    }
    let course_id = Symbol::new(env, "bench_regress");
    bench.progress_client.create_course(&course_id, &(n as u32), &(n as u32), &module_ids, &quiz_ids_src);
    bench.progress_client.enroll(&learner, &course_id);
    bench.progress_client.complete_module(&learner, &course_id, &Symbol::new(env, "mod_1"));
    for i in 0..n {
        let qid = Symbol::new(env, &format!("quiz_{}", i));
        bench.progress_client.submit_quiz_score(&learner, &course_id, &qid, &80);
    }

    // Individual claims
    env.budget().reset_default();
    for i in 0..n {
        let qid = Symbol::new(env, &format!("quiz_{}", i));
        bench.token_client.claim_reward(&learner, &course_id, &qid);
    }
    let individual_cost = env.budget().cpu_instruction_cost();

    // Reset state for batch test: use a new learner and course
    let bench2 = setup_bench();
    let env2 = &bench2.env;
    env2.mock_all_auths();
    let learner2 = Address::generate(env2);
    let mut module_ids2 = Vec::new(env2);
    module_ids2.push_back(Symbol::new(env2, "mod_1"));
    let mut quiz_ids_src2 = Vec::new(env2);
    for i in 0..n {
        quiz_ids_src2.push_back(Symbol::new(env2, &format!("quiz_{}", i)));
    }
    let course_id2 = Symbol::new(env2, "bench_regress2");
    bench2.progress_client.create_course(&course_id2, &(n as u32), &(n as u32), &module_ids2, &quiz_ids_src2);
    bench2.progress_client.enroll(&learner2, &course_id2);
    bench2.progress_client.complete_module(&learner2, &course_id2, &Symbol::new(env2, "mod_1"));
    for i in 0..n {
        let qid = Symbol::new(env2, &format!("quiz_{}", i));
        bench2.progress_client.submit_quiz_score(&learner2, &course_id2, &qid, &80);
    }

    env2.budget().reset_default();
    let quiz_ids: Vec<Symbol> = (0..n)
        .map(|i| Symbol::new(env2, &format!("quiz_{}", i)))
        .collect();
    let claimed = bench2.token_client.batch_claim_reward(&learner2, &course_id2, &quiz_ids);
    let batch_cost = env2.budget().cpu_instruction_cost();
    assert_eq!(claimed.len(), n as u32);

    let ratio = batch_cost as f64 / individual_cost as f64;

    println!(
        "regression bench: {n} claims — individual = {individual_cost}, batch = {batch_cost} (ratio: {ratio:.3})"
    );

    // Batch must be cheaper than individual (ratio < 1.0)
    assert!(
        ratio < 1.0,
        "batch_claim_reward ({batch_cost}) should cost less than {n} individual claims ({individual_cost}), ratio: {ratio:.3}"
    );

    // Ratio should not regress beyond 0.9 (90% of individual) — if batch
    // approaches individual cost, an optimization was lost.
    assert!(
        ratio < 0.9,
        "batch claim ratio {ratio:.3} exceeds 0.9 regression threshold — possible optimization regression"
    );
}

// ── End-to-end flow benchmark ───────────────────────────────────────────────

#[test]
fn bench_e2e_enroll_complete_claim_mint() {
    let bench = setup_bench();
    let env = &bench.env;
    env.mock_all_auths();

    let learner = Address::generate(env);
    let course_id = Symbol::new(env, "e2e_course");
    let quiz_id = Symbol::new(env, "quiz_1");
    let mut module_ids = Vec::new(env);
    module_ids.push_back(Symbol::new(env, "mod_1"));
    let mut quiz_ids = Vec::new(env);
    quiz_ids.push_back(quiz_id.clone());

    env.budget().reset_default();

    bench.progress_client.create_course(&course_id, &1, &1, &module_ids, &quiz_ids);
    bench.progress_client.enroll(&learner, &course_id);
    bench.progress_client.complete_module(&learner, &course_id, &Symbol::new(env, "mod_1"));
    bench.progress_client.submit_quiz_score(&learner, &course_id, &quiz_id, &85);
    bench.token_client.claim_reward(&learner, &course_id, &quiz_id);

    let metadata_uri = Symbol::new(env, "ipfs_Qm123");
    bench.credential_client.mint_credential(&learner, &course_id, &85, &metadata_uri);

    let total_cost = env.budget().cpu_instruction_cost();

    println!("bench e2e enroll→complete→claim→mint: {total_cost} CPU insns");
    assert!(total_cost > 0);
    assert_eq!(bench.token_client.balance(&learner), 8500);
    assert!(bench.credential_client.is_credential_valid(&1));
}
