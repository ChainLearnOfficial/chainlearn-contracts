/**
 * JavaScript client example for interacting with ChainLearn contracts.
 *
 * This example uses the Stellar SDK to invoke Soroban smart contracts.
 *
 * Prerequisites:
 *   npm install @stellar/stellar-sdk
 *
 * Run with:
 *   node examples/javascript_client.js
 *
 * Environment variables:
 *   STELLAR_RPC_URL        - Soroban RPC endpoint (default: testnet)
 *   STELLAR_SECRET_KEY      - Admin/learner secret key
 *   PROGRESS_TRACKER_ID     - Deployed progress-tracker contract ID
 *   LEARN_TOKEN_ID          - Deployed learn-token contract ID
 *   CREDENTIAL_NFT_ID       - Deployed credential-nft contract ID
 */

import * as StellarSdk from "@stellar/stellar-sdk";

const RPC_URL =
  process.env.STELLAR_RPC_URL || "https://soroban-testnet.stellar.org:443";
const NETWORK_PASSPHRASE = "Test SDF Network ; September 2015";

const server = new StellarSdk.SorobanRpc.Server(RPC_URL, {
  allowHttp: RPC_URL.startsWith("http"),
});

const progressTrackerId = process.env.PROGRESS_TRACKER_ID;
const learnTokenId = process.env.LEARN_TOKEN_ID;
const credentialNftId = process.env.CREDENTIAL_NFT_ID;

/**
 * Build and send a Soroban contract invocation transaction.
 */
async function invokeContract(contractId, method, args, sourceKeypair) {
  const contract = new StellarSdk.Contract(contractId);
  let txBuilder = await server.getTransactionBuilder(
    sourceKeypair,
    await server.getNetworkPassphrase()
  );

  const op = contract.call(method, ...args);
  txBuilder = txBuilder.addOperation(op);
  txBuilder = txBuilder.setTimeout(300);

  const tx = txBuilder.build();
  tx.sign(sourceKeypair);

  const result = await server.sendTransaction(tx);
  if (result.status === "ERROR") {
    throw new Error(`Transaction failed: ${JSON.stringify(result.errorResult)}`);
  }

  // Wait for confirmation
  let txResponse = await server.getTransaction(result.hash);
  while (
    txResponse.status === "NOT_FOUND" ||
    txResponse.status === "PENDING"
  ) {
    await new Promise((resolve) => setTimeout(resolve, 1000));
    txResponse = await server.getTransaction(result.hash);
  }

  if (txResponse.status === "FAILED") {
    throw new Error(`Transaction failed: ${JSON.stringify(txResponse)}`);
  }

  return txResponse;
}

// ── 1. Admin: Create a Course ─────────────────────────────────────────

async function createCourse(adminKeypair) {
  console.log("Creating course rust_101...");

  await invokeContract(
    progressTrackerId,
    "create_course",
    [
      new StellarSdk.nativeToScVal("rust_101", { type: "symbol" }),
      new StellarSdk.nativeToScVal(3, { type: "u32" }), // total_modules
      new StellarSdk.nativeToScVal(2, { type: "u32" }), // total_quizzes
      new StellarSdk.nativeToScVal(["mod_basics", "mod_ownership", "mod_traits"], {
        type: "vec",
        subtype: "symbol",
      }),
      new StellarSdk.nativeToScVal(["quiz_1", "quiz_2"], {
        type: "vec",
        subtype: "symbol",
      }),
    ],
    adminKeypair
  );

  console.log("Course created!");
}

// ── 2. Learner: Enroll ────────────────────────────────────────────────

async function enroll(learnerKeypair) {
  console.log("Enrolling in rust_101...");

  await invokeContract(
    progressTrackerId,
    "enroll",
    [
      new StellarSdk.nativeToScVal(learnerKeypair.publicKey(), {
        type: "address",
      }),
      new StellarSdk.nativeToScVal("rust_101", { type: "symbol" }),
    ],
    learnerKeypair
  );

  console.log("Enrolled!");
}

// ── 3. Learner: Complete Modules ──────────────────────────────────────

async function completeModules(learnerKeypair) {
  const modules = ["mod_basics", "mod_ownership", "mod_traits"];

  for (const mod of modules) {
    console.log(`Completing module ${mod}...`);
    await invokeContract(
      progressTrackerId,
      "complete_module",
      [
        new StellarSdk.nativeToScVal(learnerKeypair.publicKey(), {
          type: "address",
        }),
        new StellarSdk.nativeToScVal("rust_101", { type: "symbol" }),
        new StellarSdk.nativeToScVal(mod, { type: "symbol" }),
      ],
      learnerKeypair
    );
  }

  console.log("All modules completed!");
}

// ── 4. Learner: Submit Quiz Scores ────────────────────────────────────

async function submitQuizScores(learnerKeypair) {
  const quizzes = [
    { id: "quiz_1", score: 85 },
    { id: "quiz_2", score: 92 },
  ];

  for (const quiz of quizzes) {
    console.log(`Submitting score ${quiz.score} for ${quiz.id}...`);
    await invokeContract(
      progressTrackerId,
      "submit_quiz_score",
      [
        new StellarSdk.nativeToScVal(learnerKeypair.publicKey(), {
          type: "address",
        }),
        new StellarSdk.nativeToScVal("rust_101", { type: "symbol" }),
        new StellarSdk.nativeToScVal(quiz.id, { type: "symbol" }),
        new StellarSdk.nativeToScVal(quiz.score, { type: "u32" }),
      ],
      learnerKeypair
    );
  }

  console.log("Quiz scores submitted!");
}

// ── 5. Learner: Claim Token Rewards ──────────────────────────────────

async function claimRewards(learnerKeypair) {
  const quizzes = ["quiz_1", "quiz_2"];

  for (const quizId of quizzes) {
    console.log(`Claiming reward for ${quizId}...`);
    await invokeContract(
      learnTokenId,
      "claim_reward",
      [
        new StellarSdk.nativeToScVal(learnerKeypair.publicKey(), {
          type: "address",
        }),
        new StellarSdk.nativeToScVal("rust_101", { type: "symbol" }),
        new StellarSdk.nativeToScVal(quizId, { type: "symbol" }),
      ],
      learnerKeypair
    );
  }

  console.log("Rewards claimed! (100 tokens per quiz point)");
}

// ── 6. Admin: Mint Credential NFT ────────────────────────────────────

async function mintCredential(adminKeypair, learnerKeypair) {
  // Score must match the verified course average from progress-tracker
  // (85 + 92) / 2 = 88
  console.log("Minting credential NFT...");

  await invokeContract(
    credentialNftId,
    "mint_credential",
    [
      new StellarSdk.nativeToScVal(learnerKeypair.publicKey(), {
        type: "address",
      }),
      new StellarSdk.nativeToScVal("rust_101", { type: "symbol" }),
      new StellarSdk.nativeToScVal(88, { type: "u32" }), // verified average
      new StellarSdk.nativeToScVal("ipfs://Qm1234567890abcdef", {
        type: "symbol",
      }),
    ],
    adminKeypair
  );

  console.log("Credential minted!");
}

// ── Main Flow ─────────────────────────────────────────────────────────

async function main() {
  const adminKeypair = StellarSdk.Keypair.fromSecret(
    process.env.STELLAR_SECRET_KEY
  );
  const learnerKeypair = StellarSdk.Keypair.fromSecret(
    process.env.STELLAR_SECRET_KEY // In production, use a different key
  );

  console.log("=== ChainLearn Full Flow ===\n");

  await createCourse(adminKeypair);
  await enroll(learnerKeypair);
  await completeModules(learnerKeypair);
  await submitQuizScores(learnerKeypair);
  await claimRewards(learnerKeypair);
  await mintCredential(adminKeypair, learnerKeypair);

  console.log("\n=== Flow Complete ===");
}

main().catch(console.error);
