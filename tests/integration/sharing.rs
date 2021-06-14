//! Tests focused on sharing.

use rand::{seq::IteratorRandom, thread_rng, Rng};
use rand_core::{CryptoRng, RngCore};

use crate::assert_ct_eq;
use elgamal_with_sharing::{
    sharing::{
        ActiveParticipant, DecryptionShare, Params, PartialPublicKeySet, PublicKeySet,
        StartingParticipant,
    },
    DecryptionLookupTable, EncryptedChoice, Encryption, Group,
};

struct Rig<G: Group> {
    info: PublicKeySet<G>,
    participants: Vec<ActiveParticipant<G>>,
}

impl<G: Group> Rig<G> {
    fn new(params: Params, rng: &mut (impl RngCore + CryptoRng)) -> Self {
        let participants: Vec<_> = (0..params.shares)
            .map(|i| StartingParticipant::<G>::new(params, i, rng))
            .collect();

        let mut partial_info = PartialPublicKeySet::<G>::new(params);
        for (i, participant) in participants.iter().enumerate() {
            let (poly, proof) = participant.public_info();
            partial_info.add_participant(i, poly, &proof).unwrap();
        }
        let info = partial_info.complete().unwrap();

        let mut participants: Vec<_> = participants
            .into_iter()
            .map(|participant| participant.finalize_key_set(&partial_info).unwrap())
            .collect();
        for i in 0..participants.len() {
            for j in 0..participants.len() {
                if j != i {
                    let message = participants[i].message(j);
                    participants[j].receive_message(i, message).unwrap();
                }
            }
        }
        let participants = participants
            .into_iter()
            .map(|participant| participant.complete().map_err(drop).unwrap())
            .collect();
        Self { info, participants }
    }

    fn decryption_shares(
        &self,
        encryption: Encryption<G>,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> Vec<DecryptionShare<G>> {
        self.participants
            .iter()
            .map(|participant| participant.decrypt_share(encryption, rng).0)
            .collect()
    }
}

fn test_group_info_can_be_restored_from_participants<G: Group>() {
    let params = Params::new(10, 7);
    let rig: Rig<G> = Rig::new(params, &mut thread_rng());
    let expected_shared_key = rig.info.shared_key();
    let restored_info =
        PublicKeySet::from_participants(params, rig.info.participant_keys().to_vec());
    assert_eq!(restored_info.shared_key(), expected_shared_key);
}

fn tiny_fuzz<G: Group>(params: Params) {
    let mut rng = thread_rng();
    let rig: Rig<G> = Rig::new(params, &mut rng);
    for _ in 0..20 {
        let value = G::scalar_mul_basepoint(&G::generate_scalar(&mut rng));
        let encrypted = Encryption::new(value, rig.info.shared_key(), &mut rng);
        let shares = rig.decryption_shares(encrypted, &mut rng);
        for _ in 0..5 {
            let chosen_shares = shares
                .iter()
                .cloned()
                .enumerate()
                .choose_multiple(&mut rng, params.threshold);
            let decrypted = DecryptionShare::combine(params, encrypted, chosen_shares);
            assert_ct_eq(&decrypted.unwrap(), &value);
        }
    }
}

fn test_simple_voting<G: Group>() {
    const CHOICE_COUNT: usize = 5;
    const VOTES: usize = 50;

    let lookup_table = DecryptionLookupTable::<G>::new(0..=(VOTES as u64));
    let mut rng = thread_rng();
    let params = Params::new(10, 7);
    let rig = Rig::<G>::new(params, &mut rng);
    let shared_key = rig.info.shared_key();

    let mut expected_totals = [0; CHOICE_COUNT];
    let mut encrypted_totals = [Encryption::zero(); CHOICE_COUNT];

    for _ in 0..VOTES {
        let choice = rng.gen_range(0..CHOICE_COUNT);
        expected_totals[choice] += 1;
        let choice = EncryptedChoice::new(CHOICE_COUNT, choice, shared_key, &mut rng);
        assert!(choice.verify(shared_key).is_some());

        for (i, variant) in choice.variants_unchecked().iter().enumerate() {
            encrypted_totals[i] += *variant;
        }
    }

    for (&variant_totals, &expected) in encrypted_totals.iter().zip(&expected_totals) {
        // Now, each counter produces a decryption share. We take 8 shares randomly
        // (slightly more than the necessary 7).
        let decryption_shares = rig.decryption_shares(variant_totals, &mut rng);
        let decryption_shares = decryption_shares
            .into_iter()
            .enumerate()
            .choose_multiple(&mut rng, 8);
        let variant_votes =
            DecryptionShare::combine(params, variant_totals, decryption_shares).unwrap();
        let variant_votes = lookup_table.get(&variant_votes).unwrap();
        assert_eq!(variant_votes, expected);
    }
}

mod edwards {
    use super::*;
    use elgamal_with_sharing::Edwards;

    #[test]
    fn group_info_can_be_restored_from_participants() {
        test_group_info_can_be_restored_from_participants::<Edwards>();
    }

    #[test]
    fn fuzz_3_of_5() {
        tiny_fuzz::<Edwards>(Params::new(5, 3));
    }

    #[test]
    fn fuzz_4_of_5() {
        tiny_fuzz::<Edwards>(Params::new(5, 4));
    }

    #[test]
    fn fuzz_5_of_5() {
        tiny_fuzz::<Edwards>(Params::new(5, 5));
    }

    #[test]
    fn fuzz_6_of_10() {
        tiny_fuzz::<Edwards>(Params::new(10, 6));
    }

    #[test]
    fn fuzz_7_of_10() {
        tiny_fuzz::<Edwards>(Params::new(10, 7));
    }

    #[test]
    fn fuzz_8_of_10() {
        tiny_fuzz::<Edwards>(Params::new(10, 8));
    }

    #[test]
    fn fuzz_9_of_10() {
        tiny_fuzz::<Edwards>(Params::new(10, 9));
    }

    #[test]
    fn fuzz_10_of_10() {
        tiny_fuzz::<Edwards>(Params::new(10, 10));
    }

    #[test]
    fn fuzz_10_of_15() {
        tiny_fuzz::<Edwards>(Params::new(15, 10));
    }

    #[test]
    fn fuzz_12_of_15() {
        tiny_fuzz::<Edwards>(Params::new(15, 12));
    }

    #[test]
    fn fuzz_12_of_20() {
        tiny_fuzz::<Edwards>(Params::new(20, 12));
    }

    #[test]
    fn fuzz_16_of_20() {
        tiny_fuzz::<Edwards>(Params::new(20, 16));
    }

    #[test]
    fn fuzz_18_of_20() {
        tiny_fuzz::<Edwards>(Params::new(20, 18));
    }

    #[test]
    fn simple_voting() {
        test_simple_voting::<Edwards>();
    }
}

mod ristretto {
    use super::*;
    use elgamal_with_sharing::Ristretto;

    #[test]
    fn group_info_can_be_restored_from_participants() {
        test_group_info_can_be_restored_from_participants::<Ristretto>();
    }

    #[test]
    fn fuzz_3_of_5() {
        tiny_fuzz::<Ristretto>(Params::new(5, 3));
    }

    #[test]
    fn fuzz_4_of_5() {
        tiny_fuzz::<Ristretto>(Params::new(5, 4));
    }

    #[test]
    fn fuzz_5_of_5() {
        tiny_fuzz::<Ristretto>(Params::new(5, 5));
    }

    #[test]
    fn fuzz_6_of_10() {
        tiny_fuzz::<Ristretto>(Params::new(10, 6));
    }

    #[test]
    fn fuzz_7_of_10() {
        tiny_fuzz::<Ristretto>(Params::new(10, 7));
    }

    #[test]
    fn fuzz_8_of_10() {
        tiny_fuzz::<Ristretto>(Params::new(10, 8));
    }

    #[test]
    fn fuzz_9_of_10() {
        tiny_fuzz::<Ristretto>(Params::new(10, 9));
    }

    #[test]
    fn fuzz_10_of_10() {
        tiny_fuzz::<Ristretto>(Params::new(10, 10));
    }

    #[test]
    fn fuzz_10_of_15() {
        tiny_fuzz::<Ristretto>(Params::new(15, 10));
    }

    #[test]
    fn fuzz_12_of_15() {
        tiny_fuzz::<Ristretto>(Params::new(15, 12));
    }

    #[test]
    fn fuzz_12_of_20() {
        tiny_fuzz::<Ristretto>(Params::new(20, 12));
    }

    #[test]
    fn fuzz_16_of_20() {
        tiny_fuzz::<Ristretto>(Params::new(20, 16));
    }

    #[test]
    fn fuzz_18_of_20() {
        tiny_fuzz::<Ristretto>(Params::new(20, 18));
    }

    #[test]
    fn simple_voting() {
        test_simple_voting::<Ristretto>();
    }
}

mod k256 {
    use super::*;
    use elgamal_with_sharing::Generic;

    type K256 = Generic<::k256::Secp256k1>;

    #[test]
    fn group_info_can_be_restored_from_participants() {
        test_group_info_can_be_restored_from_participants::<K256>();
    }

    #[test]
    fn fuzz_3_of_5() {
        tiny_fuzz::<K256>(Params::new(5, 3));
    }

    #[test]
    fn fuzz_4_of_5() {
        tiny_fuzz::<K256>(Params::new(5, 4));
    }

    #[test]
    fn fuzz_5_of_5() {
        tiny_fuzz::<K256>(Params::new(5, 5));
    }

    #[test]
    fn fuzz_6_of_10() {
        tiny_fuzz::<K256>(Params::new(10, 6));
    }

    #[test]
    fn fuzz_7_of_10() {
        tiny_fuzz::<K256>(Params::new(10, 7));
    }

    #[test]
    fn fuzz_8_of_10() {
        tiny_fuzz::<K256>(Params::new(10, 8));
    }

    #[test]
    fn fuzz_9_of_10() {
        tiny_fuzz::<K256>(Params::new(10, 9));
    }

    #[test]
    fn fuzz_10_of_10() {
        tiny_fuzz::<K256>(Params::new(10, 10));
    }

    #[test]
    fn fuzz_10_of_15() {
        tiny_fuzz::<K256>(Params::new(15, 10));
    }

    #[test]
    fn fuzz_12_of_15() {
        tiny_fuzz::<K256>(Params::new(15, 12));
    }

    #[test]
    fn fuzz_12_of_20() {
        tiny_fuzz::<K256>(Params::new(20, 12));
    }

    #[test]
    fn fuzz_16_of_20() {
        tiny_fuzz::<K256>(Params::new(20, 16));
    }

    #[test]
    fn fuzz_18_of_20() {
        tiny_fuzz::<K256>(Params::new(20, 18));
    }

    #[test]
    fn simple_voting() {
        test_simple_voting::<K256>();
    }
}