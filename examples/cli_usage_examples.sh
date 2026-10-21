#!/bin/bash
# Example Soroban CLI commands for ChainLearn
#
# Set STELLAR_SECRET_KEY in the environment before running this script.
# This file must not export a placeholder; that would overwrite a real key.

PROGRESS_ID="C..."
TOKEN_ID="C..."
CREDENTIAL_ID="C..."
LEARNER_ADDRESS="G..."

# 1. Enroll in a course
soroban contract invoke --id $PROGRESS_ID \
    --source $STELLAR_SECRET_KEY --network testnet \
    -- enroll --learner $LEARNER_ADDRESS --course_id "rust_101"

# 2. Complete a module
soroban contract invoke --id $PROGRESS_ID \
    --source $STELLAR_SECRET_KEY --network testnet \
    -- complete_module --learner $LEARNER_ADDRESS --course_id "rust_101" --module_id "mod_basics"

# 3. Submit a quiz score
soroban contract invoke --id $PROGRESS_ID \
    --source $STELLAR_SECRET_KEY --network testnet \
    -- submit_quiz_score --learner $LEARNER_ADDRESS --course_id "rust_101" --quiz_id "quiz_1" --score 85

# 4. Claim token reward
soroban contract invoke --id $TOKEN_ID \
    --source $STELLAR_SECRET_KEY --network testnet \
    -- claim_reward --learner $LEARNER_ADDRESS --course_id "rust_101" --quiz_id "quiz_1"

# 5. Mint credential
soroban contract invoke --id $CREDENTIAL_ID \
    --source $STELLAR_SECRET_KEY --network testnet \
    -- mint_credential --caller $ADMIN_ADDRESS --to $LEARNER_ADDRESS --course_id "rust_101" --score 85 --metadata_uri "ipfs://Qm..."
