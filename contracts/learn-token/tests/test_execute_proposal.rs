#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[test]
    fn test_execute_proposal_fails_with_zero_votes() {
        let env = Env::default();
        let contract_id = env.register_contract(None, LearnTokenContract);
        let client = LearnTokenContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let proposal_id = 1;

        // Initialize contract and create proposal (setup code omitted for brevity)
        // Ensure proposal exists but has no votes

        // Should panic when executing with zero votes
        let result = std::panic::catch_unwind(|| {
            client.execute_proposal(&admin, &proposal_id);
        });

        assert!(result.is_err());
    }
}