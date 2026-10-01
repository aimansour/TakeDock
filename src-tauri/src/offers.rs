use std::collections::VecDeque;

/// Immutable, bounded offers: another window's check cannot replace approval.
pub struct Offers<T> {
    entries: VecDeque<(String, T)>,
}
impl<T> Default for Offers<T> {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
        }
    }
}
impl<T> Offers<T> {
    pub fn insert(&mut self, value: T) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        self.entries.push_back((token.clone(), value));
        if self.entries.len() > 20 {
            self.entries.pop_front();
        }
        token
    }
    pub fn take(&mut self, token: &str) -> Result<T, String> {
        let position = self
            .entries
            .iter()
            .position(|(key, _)| key == token)
            .ok_or("update_offer_expired")?;
        Ok(self.entries.remove(position).unwrap().1)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interleaved_checks_install_only_the_approved_offer() {
        let mut offers = Offers::default();
        let first = offers.insert("version X");
        let second = offers.insert("version Y");
        assert_eq!(offers.take(&first).unwrap(), "version X");
        assert!(offers.take(&first).is_err());
        assert_eq!(offers.take(&second).unwrap(), "version Y");
    }
    #[test]
    fn evicted_or_unknown_offers_fail_without_substituting_a_release() {
        let mut offers = Offers::default();
        let expired = offers.insert(0);
        for number in 1..=20 {
            offers.insert(number);
        }
        assert!(offers.take(&expired).is_err());
        assert!(offers.take("unknown").is_err());
        assert_eq!(offers.entries.len(), 20);
    }
}
