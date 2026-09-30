// ... (existing code up to line 1725 remains unchanged)

        // Tally votes
        let mut winning_choice = 0;
        let mut winning_votes = 0;
        for (choice, &votes) in vote_totals.iter().enumerate() {
            if votes > winning_votes {
                winning_votes = votes;
                winning_choice = choice;
            }
        }

        if winning_votes == 0 {
            panic!("no votes cast");
        }

        // ... (rest of the function remains unchanged)