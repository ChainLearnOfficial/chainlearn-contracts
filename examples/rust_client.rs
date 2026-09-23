//! Rust client example for interacting with ChainLearn contracts.
//!
//! This example demonstrates the full learner journey:
//! 1. Creating a course (admin)
//! 2. Enrolling a learner
//! 3. Completing modules and submitting quiz scores
//! 4. Claiming token rewards
//! 5. Minting a credential NFT
//!
//! Prerequisites:
//! - soroban-cli v21+
//! - Contracts deployed and initialized (see scripts/deploy.sh and scripts/initialize.sh)
//!
//! Run with:
//!   cargo run --example rust_client

use soroban_sdk::{Address, Env, Symbol};

// Replace with your deployed contract IDs
const PROGRESS_TRACKER_ID: &str = "C...";
const LEARN_TOKEN_ID: &str = "C...";
const CREDENTIAL_NFT_ID: &str = "C...";

fn main() {
    let env = Env::default();
    let admin = Address::from_str(&env, "S...");
    let learner = Address::from_str(&env, "G...");

    let progress_tracker = Address::from_str(&env, PROGRESS_TRACKER_ID);
    let learn_token = Address::from_str(&env, LEARN_TOKEN_ID);
    let credential_nft = Address::from_str(&env, CREDENTIAL_NFT_ID);

    // ── Admin: Create a course ─────────────────────────────────────────
    let course_id = Symbol::new(&env, "rust_101");
    let mut module_ids = soroban_sdk::Vec::new(&env);
    module_ids.push_back(Symbol::new(&env, "mod_basics"));
    module_ids.push_back(Symbol::new(&env, "mod_ownership"));
    module_ids.push_back(Symbol::new(&env, "mod_traits"));

    let mut quiz_ids = soroban_sdk::Vec::new(&env);
    quiz_ids.push_back(Symbol::new(&env, "quiz_1"));
    quiz_ids.push_back(Symbol::new(&env, "quiz_2"));

    // via CLI: soroban contract invoke --id <PROGRESS_ID> -- create_course \
    //     --course_id "rust_101" --total_modules 3 --total_quizzes 2 \
    //     --module_ids '["mod_basics","mod_ownership","mod_traits"]' \
    //     --quiz_ids '["quiz_1","quiz_2"]'

    // ── Learner: Enroll ────────────────────────────────────────────────
    // via CLI: soroban contract invoke --id <PROGRESS_ID> -- enroll \
    //     --learner <LEARNER> --course_id "rust_101"

    // ── Learner: Complete modules ──────────────────────────────────────
    // via CLI: soroban contract invoke --id <PROGRESS_ID> -- complete_module \
    //     --learner <LEARNER> --course_id "rust_101" --module_id "mod_basics"
    //
    // Modules must be completed in order (sequential enforcement).

    // Or batch complete:
    // via CLI: soroban contract invoke --id <PROGRESS_ID> -- batch_complete_module \
    //     --learner <LEARNER> --course_id "rust_101" \
    //     --module_ids '["mod_ownership","mod_traits"]'

    // ── Learner: Submit quiz scores ────────────────────────────────────
    // via CLI: soroban contract invoke --id <PROGRESS_ID> -- submit_quiz_score \
    //     --learner <LEARNER> --course_id "rust_101" --quiz_id "quiz_1" --score 85

    // Or batch submit:
    // via CLI: soroban contract invoke --id <PROGRESS_ID> -- batch_submit_quiz_score \
    //     --learner <LEARNER> --course_id "rust_101" \
    //     --quiz_scores '[["quiz_1",85],["quiz_2",92]]'

    // ── Learner: Check progress ────────────────────────────────────────
    // via CLI: soroban contract invoke --id <PROGRESS_ID> -- get_progress \
    //     --learner <LEARNER> --course_id "rust_101"
    //
    // Returns ProgressInfo with overall_progress, eligible_for_credential, etc.

    // ── Learner: Claim token reward ────────────────────────────────────
    // The reward amount is automatically calculated as: score * 100
    // via CLI: soroban contract invoke --id <TOKEN_ID> -- claim_reward \
    //     --learner <LEARNER> --course_id "rust_101" --quiz_id "quiz_1"

    // Or batch claim:
    // via CLI: soroban contract invoke --id <TOKEN_ID> -- batch_claim_reward \
    //     --learner <LEARNER> --course_id "rust_101" \
    //     --quiz_ids '["quiz_1","quiz_2"]'

    // ── Learner: Preview a claim (read-only) ──────────────────────────
    // via CLI: soroban contract invoke --id <TOKEN_ID> -- estimate_claim_gas \
    //     --learner <LEARNER> --course_id "rust_101" --quiz_id "quiz_1"
    //
    // Returns ClaimEstimate with would_succeed, estimated_reward, failure_reason.

    // ── Admin: Mint credential NFT ─────────────────────────────────────
    // Score must match the verified course average from progress-tracker.
    // via CLI: soroban contract invoke --id <CREDENTIAL_ID> -- mint_credential \
    //     --to <LEARNER> --course_id "rust_101" --score 88 \
    //     --metadata_uri "ipfs://Qm123..."

    // ── Verify credential ──────────────────────────────────────────────
    // via CLI: soroban contract invoke --id <CREDENTIAL_ID> -- verify_credential \
    //     --credential_id 1
    //
    // Returns CredentialInfo with learner, course_id, score, issued_at, revoked, metadata_uri.

    // ── Query learner credentials ──────────────────────────────────────
    // via CLI: soroban contract invoke --id <CREDENTIAL_ID> -- get_credential_count \
    //     --learner <LEARNER>
    //
    // via CLI: soroban contract invoke --id <CREDENTIAL_ID> -- get_credentials_for \
    //     --learner <LEARNER> --start 0 --limit 50

    // ── Check token balance ────────────────────────────────────────────
    // via CLI: soroban contract invoke --id <TOKEN_ID> -- balance \
    //     --address <LEARNER>

    // ── View claim history ─────────────────────────────────────────────
    // via CLI: soroban contract invoke --id <TOKEN_ID> -- get_claim_history \
    //     --learner <LEARNER>

    println!("See comments for CLI examples of the full learner journey.");
}
