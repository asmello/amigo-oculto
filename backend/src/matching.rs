use crate::models::Participant;
use crate::token::ParticipantId;
use anyhow::{anyhow, Result};
use rand::seq::SliceRandom;
use rand::thread_rng;

/// Generate random matches ensuring nobody draws themselves
pub fn generate_matches(participants: &[Participant]) -> Result<Vec<(ParticipantId, ParticipantId)>> {
    if participants.len() < 2 {
        return Err(anyhow!(
            "Precisa de pelo menos 2 participantes para fazer o sorteio"
        ));
    }

    let mut rng = thread_rng();
    let participant_ids: Vec<ParticipantId> = participants.iter().map(|p| p.id).collect();

    // Try up to 100 times to generate a valid matching
    for _ in 0..100 {
        let mut shuffled = participant_ids.clone();
        shuffled.shuffle(&mut rng);

        // Check if anyone drew themselves
        let valid = participant_ids
            .iter()
            .zip(shuffled.iter())
            .all(|(giver, receiver)| giver != receiver);

        if valid {
            // Create matches: (giver_id, receiver_id)
            return Ok(participant_ids
                .iter()
                .zip(shuffled.iter())
                .map(|(giver, receiver)| (*giver, *receiver))
                .collect());
        }
    }

    Err(anyhow!(
        "Não foi possível gerar um sorteio válido após 100 tentativas"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Participant;
    use crate::token::{GameId, ViewToken};

    fn create_test_participant(name: &str) -> Participant {
        Participant {
            id: ParticipantId::new(),
            game_id: GameId::new(),
            name: name.to_string(),
            email: format!("{}@test.com", name),
            matched_with_id: None,
            view_token: ViewToken::generate(),
            has_viewed: false,
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_generate_matches_success() {
        let participants = vec![
            create_test_participant("Alice"),
            create_test_participant("Bob"),
            create_test_participant("Charlie"),
            create_test_participant("Diana"),
        ];

        let matches = generate_matches(&participants).unwrap();

        assert_eq!(matches.len(), 4);

        // Verify nobody drew themselves
        for (giver, receiver) in &matches {
            assert_ne!(giver, receiver);
        }

        // Verify all participants are givers
        let givers: Vec<_> = matches.iter().map(|(g, _)| g).collect();
        for p in &participants {
            assert!(givers.contains(&&p.id));
        }

        // Verify all participants are receivers
        let receivers: Vec<_> = matches.iter().map(|(_, r)| r).collect();
        for p in &participants {
            assert!(receivers.contains(&&p.id));
        }
    }

    #[test]
    fn test_generate_matches_insufficient_participants() {
        let participants = vec![create_test_participant("Alice")];

        let result = generate_matches(&participants);
        assert!(result.is_err());
    }
}
