use porto_core_domain::{DomainError, TESTNET_DISCLAIMER};

#[derive(Clone, Debug, Eq, PartialEq)]
enum Environment {
    Testnet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ServiceConfig {
    environment: Environment,
    identity_provider: String,
    billing_provider: String,
    evidence_store: String,
}

impl ServiceConfig {
    fn validate(&self) -> Result<(), DomainError> {
        for provider in [
            &self.identity_provider,
            &self.billing_provider,
            &self.evidence_store,
        ] {
            if provider.trim().is_empty() {
                return Err(DomainError::InvalidConfiguration(
                    "provider names must not be empty",
                ));
            }
        }
        Ok(())
    }
}

fn main() {
    let config = ServiceConfig {
        environment: Environment::Testnet,
        identity_provider: "fixture-identity".into(),
        billing_provider: "fixture-billing".into(),
        evidence_store: "local-private-evidence".into(),
    };
    config
        .validate()
        .expect("the local fixture configuration is valid");
    println!("porto coordinator scaffold: {TESTNET_DISCLAIMER}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_configuration_requires_named_boundaries() {
        let config = ServiceConfig {
            environment: Environment::Testnet,
            identity_provider: "fixture".into(),
            billing_provider: "fixture".into(),
            evidence_store: "private".into(),
        };
        assert_eq!(config.validate(), Ok(()));
    }
}
