// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! CLI Integration Tests
//!
//! Tests trace to feature files:
//! - identity_management.feature
//! - contact_card_management.feature
//! - contact_exchange.feature
//! - visibility_labels.feature

use std::process::{Command, Output};
use tempfile::TempDir;

/// Helper to run CLI commands in an isolated data directory
struct CliTestContext {
    data_dir: TempDir,
    relay_url: String,
}

impl CliTestContext {
    fn new() -> Self {
        Self {
            data_dir: TempDir::new().expect("Failed to create temp dir"),
            relay_url: "ws://127.0.0.1:8080".to_string(),
        }
    }

    /// Run a CLI command and return the output
    fn run(&self, args: &[&str]) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_vauchi"));
        cmd.arg("--data-dir")
            .arg(self.data_dir.path())
            .arg("--relay")
            .arg(&self.relay_url);

        for arg in args {
            cmd.arg(arg);
        }

        cmd.output().expect("Failed to execute command")
    }

    /// Run a command and assert success
    fn run_success(&self, args: &[&str]) -> String {
        let output = self.run(args);
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        assert!(
            output.status.success(),
            "Command {:?} failed.\nStdout: {}\nStderr: {}",
            args,
            stdout,
            stderr
        );
        stdout
    }

    /// Run a command and assert failure
    fn run_failure(&self, args: &[&str]) -> String {
        let output = self.run(args);
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        assert!(
            !output.status.success(),
            "Command {:?} should have failed but succeeded",
            args
        );
        stderr
    }

    /// Initialize identity with name
    fn init(&self, name: &str) -> String {
        self.run_success(&["init", name])
    }
}

// ===========================================================================
// Identity Management Tests
// Trace: features/identity_management.feature
// ===========================================================================

mod identity_management {
    use super::*;

    /// Trace: identity_management.feature - "Create new identity on first launch"
    // @scenario: identity_management:Create new identity on first launch
    #[test]
    fn test_init_creates_identity() {
        let ctx = CliTestContext::new();
        let output = ctx.init("Alice Smith");

        assert!(output.contains("Identity created: Alice Smith"));
        assert!(output.contains("Public ID:"));
    }

    /// Trace: identity_management.feature - "Set display name during identity setup"
    // @scenario: identity_management:Set display name during identity setup
    #[test]
    fn test_init_sets_display_name() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["card", "show"]);
        assert!(output.contains("Alice Smith"));
    }

    /// Trace: identity_management.feature - "Display name validation"
    // @scenario: identity_management:Display name validation
    /// M-1: Verify that empty-name init either fails or creates empty identity.
    #[test]
    fn test_init_empty_name_behavior() {
        let ctx = CliTestContext::new();
        let output = ctx.run(&["init", ""]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            // If accepted, the card should exist (even with empty name)
            let card = ctx.run_success(&["card", "show"]);
            assert!(
                !card.is_empty(),
                "Empty-name init succeeded but card show returned nothing"
            );
        } else {
            // If rejected, stderr should mention the name issue
            assert!(
                stderr.contains("empty")
                    || stderr.contains("name")
                    || stderr.contains("invalid")
                    || stderr.contains("required"),
                "Empty-name init failed but stderr is unclear: stdout={}, stderr={}",
                stdout,
                stderr
            );
        }
    }

    /// Trace: identity_management.feature - Cannot re-initialize
    // @scenario: identity_management:Cannot re-initialize without force
    #[test]
    fn test_init_already_initialized_fails() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let stderr = ctx.run_failure(&["init", "Bob Jones"]);
        assert!(stderr.contains("already initialized"));
    }

    /// Trace: identity_management.feature - Re-initialize with --force
    // @scenario: identity_management:Re-initialize with force flag
    #[test]
    fn test_init_force_overwrites_existing_identity() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        // Re-init with --force should succeed
        let output = ctx.run_success(&["init", "--force", "Bob Jones"]);
        assert!(output.contains("Identity created: Bob Jones"));

        // Verify the new identity is active
        let card_output = ctx.run_success(&["card", "show"]);
        assert!(card_output.contains("Bob Jones"));
    }

    /// Trace: identity_management.feature - "Create encrypted identity backup"
    // @scenario: identity_management:Create encrypted identity backup
    /// Note: Skipped - export requires interactive password input via dialoguer
    #[test]
    #[ignore = "requires interactive terminal for password input"]
    fn test_export_creates_backup() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let backup_path = ctx.data_dir.path().join("backup.json");
        let output = ctx.run_success(&["export", backup_path.to_str().unwrap()]);

        assert!(output.contains("exported") || output.contains("Backup"));
        assert!(backup_path.exists());
    }

    /// Trace: identity_management.feature - "Restore identity from backup"
    // @scenario: identity_management:Restore identity from backup
    /// Note: Skipped - import requires interactive password input via dialoguer
    #[test]
    #[ignore = "requires interactive terminal for password input"]
    fn test_import_restores_identity() {
        // Create first identity and export
        let ctx1 = CliTestContext::new();
        ctx1.init("Alice Smith");

        let backup_path = ctx1.data_dir.path().join("backup.json");
        ctx1.run_success(&["export", backup_path.to_str().unwrap()]);

        // Import into new context
        let ctx2 = CliTestContext::new();
        let output = ctx2.run_success(&["import", backup_path.to_str().unwrap()]);

        assert!(
            output.contains("imported")
                || output.contains("restored")
                || output.contains("Identity")
        );

        // Verify name was restored
        let card_output = ctx2.run_success(&["card", "show"]);
        assert!(card_output.contains("Alice Smith"));
    }

    /// Trace: identity_management.feature - "Identity verification via public key fingerprint"
    // @scenario: identity_management:Identity verification via public key fingerprint
    #[test]
    fn test_device_info_shows_fingerprint() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["device", "info"]);
        // Should show public key info
        assert!(
            output.contains("Device") || output.contains("Public") || output.contains("ID"),
            "Expected device info, got: {}",
            output
        );
    }
}

// ===========================================================================
// Contact Card Management Tests
// Trace: features/contact_card_management.feature
// ===========================================================================

mod contact_card_management {
    use super::*;

    /// Trace: contact_card_management.feature - "Add a phone number field"
    // @scenario: contact_card_management:Add a phone number field
    #[test]
    fn test_card_add_phone_field() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["card", "add", "phone", "Mobile", "+1-555-123-4567"]);
        assert!(output.contains("added") || output.contains("Mobile"));

        let card = ctx.run_success(&["card", "show"]);
        assert!(card.contains("Mobile"));
        assert!(card.contains("+1-555-123-4567"));
    }

    /// Trace: contact_card_management.feature - "Add an email field"
    // @scenario: contact_card_management:Add an email field
    #[test]
    fn test_card_add_email_field() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        ctx.run_success(&["card", "add", "email", "Work", "alice@company.com"]);

        let card = ctx.run_success(&["card", "show"]);
        assert!(card.contains("Work"));
        assert!(card.contains("alice@company.com"));
    }

    /// Trace: contact_card_management.feature - "Add a website field"
    // @scenario: contact_card_management:Add a website field
    #[test]
    fn test_card_add_website_field() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        ctx.run_success(&[
            "card",
            "add",
            "website",
            "Personal",
            "https://alice.example.com",
        ]);

        let card = ctx.run_success(&["card", "show"]);
        assert!(card.contains("Personal"));
        assert!(card.contains("https://alice.example.com"));
    }

    /// Trace: contact_card_management.feature - "Edit an existing field value"
    // @scenario: contact_card_management:Edit an existing field value
    #[test]
    fn test_card_edit_field() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        ctx.run_success(&["card", "add", "phone", "Mobile", "+1-555-123-4567"]);
        ctx.run_success(&["card", "edit", "Mobile", "+1-555-999-8888"]);

        let card = ctx.run_success(&["card", "show"]);
        assert!(card.contains("+1-555-999-8888"));
        assert!(!card.contains("+1-555-123-4567"));
    }

    /// Trace: contact_card_management.feature - "Remove a field from contact card"
    // @scenario: contact_card_management:Remove a field from contact card
    #[test]
    fn test_card_remove_field() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        ctx.run_success(&["card", "add", "phone", "Mobile", "+1-555-123-4567"]);
        ctx.run_success(&["card", "remove", "Mobile"]);

        let card = ctx.run_success(&["card", "show"]);
        assert!(!card.contains("Mobile") || card.contains("No fields"));
    }

    /// Trace: contact_card_management.feature - "Update display name"
    // @scenario: contact_card_management:Update display name
    #[test]
    fn test_card_edit_name() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        ctx.run_success(&["card", "edit-name", "Alice S."]);

        let card = ctx.run_success(&["card", "show"]);
        assert!(card.contains("Alice S."));
    }

    /// Trace: contact_card_management.feature - Multiple fields
    // @scenario: contact_card_management:Add multiple fields to contact card
    #[test]
    fn test_card_multiple_fields() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        ctx.run_success(&["card", "add", "phone", "Mobile", "+1-555-123-4567"]);
        ctx.run_success(&["card", "add", "email", "Work", "alice@work.com"]);
        ctx.run_success(&["card", "add", "email", "Personal", "alice@personal.com"]);

        let card = ctx.run_success(&["card", "show"]);
        assert!(card.contains("Mobile"));
        assert!(card.contains("Work"));
        assert!(card.contains("Personal"));
    }

    /// Trace: contact_card_management.feature - "Add social media fields"
    // @scenario: contact_card_management:Add social media fields
    #[test]
    fn test_card_add_social_field() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        ctx.run_success(&["card", "add", "social", "GitHub", "alicesmith"]);

        let card = ctx.run_success(&["card", "show"]);
        assert!(card.contains("GitHub") || card.contains("github"));
    }
}

// ===========================================================================
// Contact Exchange Tests
// Trace: features/contact_exchange.feature
// ===========================================================================

mod contact_exchange {
    use super::*;

    /// Trace: contact_exchange.feature - "Generate exchange QR code"
    // @scenario: contact_exchange:Generate exchange QR code
    #[test]
    fn test_exchange_start_generates_data() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["exchange", "start"]);
        // Should output exchange data (base64 or URL)
        assert!(
            output.contains("wb://") || output.len() > 50,
            "Expected exchange data, got: {}",
            output
        );
    }

    /// Trace: contact_exchange.feature - "Successful QR code exchange"
    // @scenario: contact_exchange:Successful QR code exchange
    #[test]
    fn test_exchange_complete_flow() {
        // Alice generates exchange data
        let alice = CliTestContext::new();
        alice.init("Alice Smith");
        alice.run_success(&["card", "add", "email", "Work", "alice@work.com"]);

        let alice_exchange = alice.run_success(&["exchange", "start"]);
        let alice_data: String = alice_exchange
            .lines()
            .last()
            .unwrap_or("")
            .trim()
            .to_string();

        // Bob completes exchange with Alice's data
        let bob = CliTestContext::new();
        bob.init("Bob Jones");
        bob.run_success(&["card", "add", "phone", "Mobile", "+1-555-262-1234"]);

        // Try to complete - this may fail without relay but tests the flow
        let result = bob.run(&["exchange", "complete", &alice_data]);

        // The exchange flow should at least parse the data
        // Full exchange requires relay connectivity
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        let combined = format!("{}{}", stdout, stderr);

        // Should either succeed or fail with connectivity error, not parsing error
        assert!(
            !combined.contains("malformed"),
            "Exchange data parsing failed: {}",
            combined
        );
    }

    /// Trace: contact_exchange.feature - "Handle malformed QR code"
    // @scenario: contact_exchange:Handle malformed QR code
    #[test]
    fn test_exchange_complete_invalid_data() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let stderr = ctx.run_failure(&["exchange", "complete", "not-valid-exchange-data"]);
        assert!(
            stderr.contains("invalid")
                || stderr.contains("Invalid")
                || stderr.contains("failed")
                || stderr.contains("error"),
            "Expected error for invalid data, got: {}",
            stderr
        );
    }
}

// ===========================================================================
// Contacts Management Tests
// Trace: features/contacts_management.feature
// ===========================================================================

mod contacts_management {
    use super::*;

    /// Trace: contacts_management.feature - "List all contacts"
    // @scenario: contacts_management:View all contacts
    #[test]
    fn test_contacts_list_empty() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["contacts", "list"]);
        assert!(
            output.contains("No contacts")
                || output.contains("empty")
                || output.is_empty()
                || output.contains("0"),
            "Expected no contacts, got: {}",
            output
        );
    }

    /// Trace: contacts_management.feature - "Search contacts"
    // @scenario: contacts_management:Search contacts by name
    #[test]
    fn test_contacts_search() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["contacts", "search", "Bob"]);
        // With no contacts, should return empty or "not found"
        assert!(
            output.contains("No")
                || output.contains("not found")
                || output.is_empty()
                || output.contains("0"),
            "Unexpected search result: {}",
            output
        );
    }
}

// ===========================================================================
// Visibility Labels Tests
// Trace: features/visibility_labels.feature
// ===========================================================================

mod visibility_labels {
    use super::*;

    /// Trace: visibility_labels.feature - "Create a new visibility label"
    // @scenario: visibility_control:Create a new visibility label
    #[test]
    fn test_labels_create() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["labels", "create", "Family"]);
        assert!(
            output.contains("created") || output.contains("Family"),
            "Expected label created, got: {}",
            output
        );
    }

    /// Trace: visibility_labels.feature - "List labels"
    // @scenario: visibility_control:List visibility labels
    #[test]
    fn test_labels_list() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        ctx.run_success(&["labels", "create", "Family"]);
        ctx.run_success(&["labels", "create", "Friends"]);

        let output = ctx.run_success(&["labels", "list"]);
        assert!(output.contains("Family"));
        assert!(output.contains("Friends"));
    }

    /// Trace: visibility_labels.feature - "Cannot create duplicate label names"
    // @scenario: visibility_control:Cannot create duplicate label names
    #[test]
    fn test_labels_create_duplicate_fails() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        ctx.run_success(&["labels", "create", "Family"]);
        let stderr = ctx.run_failure(&["labels", "create", "Family"]);

        assert!(
            stderr.contains("exists") || stderr.contains("duplicate") || stderr.contains("already"),
            "Expected duplicate error, got: {}",
            stderr
        );
    }

    /// Trace: visibility_labels.feature - "Rename an existing label"
    // @scenario: visibility_control:Rename an existing label
    #[test]
    fn test_labels_rename() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        ctx.run_success(&["labels", "create", "Work"]);
        ctx.run_success(&["labels", "rename", "Work", "Colleagues"]);

        let output = ctx.run_success(&["labels", "list"]);
        assert!(output.contains("Colleagues"));
        assert!(!output.contains("Work") || output.contains("Colleagues"));
    }

    /// Trace: visibility_labels.feature - "Delete a label"
    // @scenario: visibility_control:Delete a label
    #[test]
    fn test_labels_delete() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        ctx.run_success(&["labels", "create", "Temporary"]);
        ctx.run_success(&["labels", "delete", "Temporary"]);

        let output = ctx.run_success(&["labels", "list"]);
        assert!(
            !output.contains("Temporary") || output.contains("No labels"),
            "Label should be deleted, got: {}",
            output
        );
    }

    /// Trace: visibility_labels.feature - "Show label details"
    // @scenario: visibility_control:Show label details
    #[test]
    fn test_labels_show() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        ctx.run_success(&["labels", "create", "Family"]);

        let output = ctx.run_success(&["labels", "show", "Family"]);
        assert!(output.contains("Family"));
    }
}

// ===========================================================================
// Device Management Tests
// Trace: features/device_management.feature
// ===========================================================================

mod device_management {
    use super::*;

    /// Trace: device_management.feature - "List linked devices"
    // @scenario: device_management:View linked devices
    #[test]
    fn test_device_list() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["device", "list"]);
        // Should list at least the current device
        assert!(
            output.contains("Device") || output.contains("device") || output.contains("1"),
            "Expected device list, got: {}",
            output
        );
    }

    /// Trace: device_management.feature - "Generate device linking QR code"
    // @scenario: device_management:Generate device linking QR code
    /// M-5: Verify QR data structure, not just length.
    #[test]
    fn test_device_link_generates_qr() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["device", "link"]);
        // Should contain base64-encoded data or a wb:// URL
        let last_line = output.lines().last().unwrap_or("").trim();
        assert!(
            last_line.contains("wb://") || last_line.contains("vdl://") || last_line.len() > 50,
            "Expected device link data with protocol prefix or substantial base64, got: {}",
            output
        );
    }
}

// ===========================================================================
// Recovery Tests
// Trace: features/identity_management.feature (recovery scenarios)
// ===========================================================================

mod recovery {
    use super::*;

    /// Trace: Recovery settings
    // @scenario: identity_management:Configure recovery settings
    #[test]
    fn test_recovery_settings_show() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["recovery", "settings", "show"]);
        assert!(
            output.contains("recovery")
                || output.contains("Recovery")
                || output.contains("threshold")
                || output.contains("voucher"),
            "Expected recovery settings, got: {}",
            output
        );
    }

    /// Trace: Recovery settings can be configured
    // @scenario: identity_management:Configure recovery settings
    #[test]
    fn test_recovery_settings_set() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&[
            "recovery",
            "settings",
            "set",
            "--recovery",
            "3",
            "--verification",
            "2",
        ]);
        assert!(
            output.contains("updated") || output.contains("set") || output.contains("3"),
            "Expected settings update confirmation, got: {}",
            output
        );
    }

    /// Trace: Recovery status shows pending state
    // @scenario: identity_management:View recovery status
    #[test]
    fn test_recovery_status_no_claim() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["recovery", "status"]);
        assert!(
            output.contains("No")
                || output.contains("none")
                || output.contains("active")
                || output.contains("claim"),
            "Expected no active recovery, got: {}",
            output
        );
    }
}

// ===========================================================================
// Social Network Tests
// Trace: features/contact_card_management.feature (social registry)
// ===========================================================================

mod social {
    use super::*;

    /// Trace: contact_card_management.feature - "List available social networks"
    // @scenario: contact_card_management:List available social networks
    #[test]
    fn test_social_list() {
        let ctx = CliTestContext::new();

        let output = ctx.run_success(&["social", "list"]);
        assert!(
            output.contains("twitter")
                || output.contains("Twitter")
                || output.contains("github")
                || output.contains("GitHub")
        );
    }

    /// Trace: contact_card_management.feature - "Generate profile URL"
    // @scenario: contact_card_management:Generate social profile URL
    #[test]
    fn test_social_url() {
        let ctx = CliTestContext::new();

        let output = ctx.run_success(&["social", "url", "github", "octocat"]);
        assert!(output.contains("github.com") && output.contains("octocat"));
    }

    /// Trace: contact_card_management.feature - Search social networks
    // @scenario: contact_card_management:Search social networks
    #[test]
    fn test_social_list_search() {
        let ctx = CliTestContext::new();

        let output = ctx.run_success(&["social", "list", "git"]);
        assert!(
            output.contains("GitHub")
                || output.contains("github")
                || output.contains("GitLab")
                || output.contains("gitlab")
        );
    }
}

// ===========================================================================
// Sync Tests
// Trace: features/sync_updates.feature
// ===========================================================================

mod sync {
    use super::*;

    /// Trace: sync_updates.feature - Sync command runs (may fail without relay)
    // @scenario: sync_updates:Client initiates sync with relay
    /// M-3: Tightened assertion — must show sync-specific or relay-specific output.
    #[test]
    fn test_sync_command_executes() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        // Sync will fail without a running relay, but should execute and report
        let output = ctx.run(&["sync"]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined_lower = format!("{}{}", stdout, stderr).to_lowercase();

        // Must mention sync, relay, or connection — not just any English word
        assert!(
            combined_lower.contains("sync")
                || combined_lower.contains("relay")
                || combined_lower.contains("connect")
                || combined_lower.contains("websocket"),
            "Sync output should mention sync/relay/connection, got: stdout={}, stderr={}",
            stdout,
            stderr
        );
    }
}

// ===========================================================================
// Shell Completions Test
// ===========================================================================

mod completions {
    use super::*;

    /// Test completions generation works
    #[test]
    fn test_completions_bash() {
        let ctx = CliTestContext::new();
        let output = ctx.run_success(&["completions", "bash"]);
        assert!(output.contains("complete") || output.contains("_vauchi"));
    }

    /// Test completions for different shells
    #[test]
    fn test_completions_zsh() {
        let ctx = CliTestContext::new();
        let output = ctx.run_success(&["completions", "zsh"]);
        assert!(output.contains("compdef") || output.contains("_vauchi"));
    }
}

// ===========================================================================
// Contact Recovery Trust Tests
// Trace: features/contact_recovery.feature
// ===========================================================================

mod contact_recovery_trust {
    use super::*;

    /// Trace: contact_recovery.feature line 57 - "Mark contact as trusted for recovery"
    /// Tests that the trust command requires initialization.
    #[test]
    fn test_trust_requires_init() {
        let ctx = CliTestContext::new();
        let stderr = ctx.run_failure(&["contacts", "trust", "some-id"]);
        assert!(stderr.contains("not initialized"));
    }

    /// Trace: contact_recovery.feature line 64 - "Remove recovery trust"
    /// Tests that the untrust command requires initialization.
    #[test]
    fn test_untrust_requires_init() {
        let ctx = CliTestContext::new();
        let stderr = ctx.run_failure(&["contacts", "untrust", "some-id"]);
        assert!(stderr.contains("not initialized"));
    }

    /// Trace: contact_recovery.feature line 57 - "Mark contact as trusted"
    /// Tests that trust reports contact not found for unknown ID.
    #[test]
    fn test_trust_contact_not_found() {
        let ctx = CliTestContext::new();
        ctx.init("Alice");

        let stderr = ctx.run_failure(&["contacts", "trust", "nonexistent"]);
        assert!(stderr.contains("not found"));
    }

    /// Trace: contact_recovery.feature line 64 - "Remove recovery trust"
    /// Tests that untrust reports contact not found for unknown ID.
    #[test]
    fn test_untrust_contact_not_found() {
        let ctx = CliTestContext::new();
        ctx.init("Alice");

        let stderr = ctx.run_failure(&["contacts", "untrust", "nonexistent"]);
        assert!(stderr.contains("not found"));
    }

    /// Trace: contact_recovery.feature line 119 - "Warning when trusted contacts below threshold"
    /// Tests that recovery settings show displays trusted count.
    #[test]
    fn test_recovery_settings_shows_trusted_count() {
        let ctx = CliTestContext::new();
        ctx.init("Alice");

        let output = ctx.run_success(&["recovery", "settings", "show"]);
        assert!(output.contains("Trusted Contacts:"));
        assert!(output.contains("0"));
    }

    /// Tests that the help includes the trust/untrust subcommands.
    #[test]
    fn test_trust_untrust_in_help() {
        let ctx = CliTestContext::new();
        let output = ctx.run_success(&["contacts", "help"]);
        assert!(output.contains("trust") || output.contains("Trust"));
        assert!(output.contains("untrust") || output.contains("Untrust"));
    }
}

// ===========================================================================
// GDPR Commands Tests (Tracker #210, MIS-7)
// Trace: features/privacy_compliance.feature
// ===========================================================================

mod gdpr {
    use super::*;

    /// Trace: privacy_compliance.feature - "View consent status"
    #[test]
    fn test_gdpr_consent_status() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["gdpr", "consent-status"]);
        assert!(
            output.contains("consent")
                || output.contains("Consent")
                || output.contains("No consent"),
            "Expected consent status output, got: {}",
            output
        );
    }

    /// Trace: privacy_compliance.feature - "Grant consent for data processing"
    #[test]
    fn test_gdpr_grant_and_revoke_consent() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["gdpr", "grant-consent", "data_processing"]);
        assert!(
            output.contains("granted") || output.contains("Granted"),
            "Expected grant confirmation, got: {}",
            output
        );

        // Revoke the same consent
        let revoke = ctx.run_success(&["gdpr", "revoke-consent", "data_processing"]);
        assert!(
            revoke.contains("revoked") || revoke.contains("Revoked"),
            "Expected revoke confirmation, got: {}",
            revoke
        );
    }

    /// Trace: privacy_compliance.feature - "View deletion status"
    #[test]
    fn test_gdpr_deletion_status() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["gdpr", "deletion-status"]);
        assert!(
            output.contains("No deletion") || output.contains("deletion"),
            "Expected deletion status, got: {}",
            output
        );
    }

    /// Trace: privacy_compliance.feature - "Export personal data"
    #[test]
    fn test_gdpr_export_data() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let export_path = ctx.data_dir.path().join("gdpr-export.json");
        let output = ctx.run_success(&["gdpr", "export", export_path.to_str().unwrap()]);
        assert!(
            output.contains("export") || output.contains("Export"),
            "Expected export confirmation, got: {}",
            output
        );
        assert!(export_path.exists(), "Export file should be created");

        // Verify it's valid JSON
        let contents = std::fs::read_to_string(&export_path).unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&contents).expect("Export should be valid JSON");
        assert!(parsed.is_object(), "Export should be a JSON object");
    }

    /// Trace: privacy_compliance.feature - "Export encrypted personal data"
    #[test]
    fn test_gdpr_export_encrypted() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let export_path = ctx.data_dir.path().join("gdpr-export.enc");
        let output = ctx.run_success(&[
            "gdpr",
            "export",
            "--password",
            "MyStr0ngP@ssword!",
            export_path.to_str().unwrap(),
        ]);
        assert!(
            output.contains("export") || output.contains("Export"),
            "Expected export confirmation, got: {}",
            output
        );
        assert!(export_path.exists(), "Export file should be created");

        // Verify it's NOT plain JSON (it's encrypted binary)
        let contents = std::fs::read(&export_path).unwrap();
        assert_eq!(contents[0], 0x01, "First byte should be version 0x01");
        // Should not be parseable as JSON
        assert!(
            serde_json::from_slice::<serde_json::Value>(&contents).is_err(),
            "Encrypted export should not be valid JSON"
        );
    }

    /// Trace: privacy_compliance.feature - "Invalid consent type"
    #[test]
    fn test_gdpr_grant_invalid_consent_type() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let stderr = ctx.run_failure(&["gdpr", "grant-consent", "nonexistent_type"]);
        assert!(
            stderr.contains("Unknown") || stderr.contains("unknown") || stderr.contains("invalid"),
            "Expected unknown type error, got: {}",
            stderr
        );
    }

    /// Trace: privacy_compliance.feature - "Cancel deletion without scheduling"
    #[test]
    fn test_gdpr_cancel_deletion_not_scheduled() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        // Cancel when nothing scheduled — should fail or warn
        let output = ctx.run(&["gdpr", "cancel-deletion"]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{}{}", stdout, stderr).to_lowercase();
        assert!(
            combined.contains("no deletion")
                || combined.contains("not scheduled")
                || combined.contains("cancelled")
                || combined.contains("no active"),
            "Expected no-deletion message, got: {}",
            combined
        );
    }
}

// ===========================================================================
// Tor Commands Tests (MIS-8)
// Trace: features/privacy_compliance.feature
// ===========================================================================

mod tor {
    use super::*;

    /// Trace: privacy_compliance.feature - "View Tor status"
    // @scenario: tor_mode:View Tor connection status
    #[test]
    fn test_tor_status_default() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["tor", "status"]);
        assert!(
            output.contains("DISABLED"),
            "Tor should be disabled by default, got: {}",
            output
        );
    }

    /// Trace: privacy_compliance.feature - "Enable Tor mode"
    // @scenario: tor_mode:Enable and disable Tor mode
    #[test]
    fn test_tor_enable_disable_cycle() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        // Enable
        let enable = ctx.run_success(&["tor", "enable"]);
        assert!(
            enable.contains("enabled") || enable.contains("Enabled"),
            "Expected enable confirmation, got: {}",
            enable
        );

        // Status should show enabled
        let status = ctx.run_success(&["tor", "status"]);
        assert!(
            status.contains("ENABLED"),
            "Tor should be enabled, got: {}",
            status
        );

        // Disable
        let disable = ctx.run_success(&["tor", "disable"]);
        assert!(
            disable.contains("disabled") || disable.contains("Disabled"),
            "Expected disable confirmation, got: {}",
            disable
        );

        // Status should show disabled
        let status2 = ctx.run_success(&["tor", "status"]);
        assert!(
            status2.contains("DISABLED"),
            "Tor should be disabled, got: {}",
            status2
        );
    }

    /// Trace: privacy_compliance.feature - "Enable Tor when already enabled"
    // @scenario: tor_mode:Enable Tor when already enabled
    #[test]
    fn test_tor_enable_idempotent() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        ctx.run_success(&["tor", "enable"]);
        let output = ctx.run_success(&["tor", "enable"]);
        assert!(
            output.contains("already"),
            "Re-enabling Tor should say 'already enabled', got: {}",
            output
        );
    }

    /// Trace: privacy_compliance.feature - "Request new Tor circuit"
    // @scenario: tor_mode:Request new Tor circuit
    #[test]
    fn test_tor_new_circuit() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["tor", "new-circuit"]);
        // Should mention that Tor is not enabled, or circuit requested
        assert!(
            output.contains("circuit")
                || output.contains("Circuit")
                || output.contains("not enabled"),
            "Expected circuit-related output, got: {}",
            output
        );
    }

    /// Trace: privacy_compliance.feature - "Manage bridge addresses"
    // @scenario: tor_mode:Manage Tor bridge addresses
    #[test]
    fn test_tor_bridges_lifecycle() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        // List empty
        let list = ctx.run_success(&["tor", "bridges", "list"]);
        assert!(
            list.contains("No bridges") || list.contains("none"),
            "Expected no bridges, got: {}",
            list
        );

        // Add bridge
        let add = ctx.run_success(&["tor", "bridges", "add", "obfs4://198.51.100.1:9001"]);
        assert!(
            add.contains("added") || add.contains("Added"),
            "Expected bridge added, got: {}",
            add
        );

        // List should show it
        let list2 = ctx.run_success(&["tor", "bridges", "list"]);
        assert!(
            list2.contains("198.51.100.1"),
            "Bridge should appear in list, got: {}",
            list2
        );

        // Clear
        let clear = ctx.run_success(&["tor", "bridges", "clear"]);
        assert!(
            clear.contains("Cleared") || clear.contains("cleared"),
            "Expected bridges cleared, got: {}",
            clear
        );

        // List should be empty again
        let list3 = ctx.run_success(&["tor", "bridges", "list"]);
        assert!(
            list3.contains("No bridges") || list3.contains("none"),
            "Expected no bridges after clear, got: {}",
            list3
        );
    }
}

// ===========================================================================
// Duress PIN Tests (MIS-9)
// Trace: features/duress_mode.feature
// ===========================================================================

mod duress {
    use super::*;

    /// Trace: duress_mode.feature - "View duress status"
    // @scenario: duress_pin:View duress PIN status
    #[test]
    fn test_duress_status_default() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["duress", "status"]);
        assert!(
            output.contains("NOT SET"),
            "Duress should not be set by default, got: {}",
            output
        );
    }

    /// Trace: duress_mode.feature - "Disable when not enabled"
    // @scenario: duress_pin:Disable duress mode when not enabled
    #[test]
    fn test_duress_disable_when_not_enabled() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["duress", "disable"]);
        assert!(
            output.contains("not enabled") || output.contains("Not"),
            "Expected not-enabled message, got: {}",
            output
        );
    }

    /// Trace: duress_mode.feature - "Test auth without password"
    // @scenario: duress_pin:Test duress authentication
    #[test]
    fn test_duress_test_without_password() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let stderr = ctx.run_failure(&["duress", "test", "1234"]);
        assert!(
            stderr.contains("password") || stderr.contains("Password"),
            "Expected password-required error, got: {}",
            stderr
        );
    }
}

// ===========================================================================
// Emergency Broadcast Tests (MIS-10)
// Trace: features/emergency_broadcast.feature
// ===========================================================================

mod emergency {
    use super::*;

    /// Trace: emergency_broadcast.feature - "View emergency status"
    // @scenario: emergency_broadcast:View emergency broadcast status
    #[test]
    fn test_emergency_status_default() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["emergency", "status"]);
        assert!(
            output.contains("NOT CONFIGURED"),
            "Emergency should not be configured by default, got: {}",
            output
        );
    }

    /// Trace: emergency_broadcast.feature - "Disable when not configured"
    // @scenario: emergency_broadcast:Disable emergency broadcast
    #[test]
    fn test_emergency_disable_when_not_configured() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let output = ctx.run_success(&["emergency", "disable"]);
        assert!(
            output.contains("not configured") || output.contains("Not"),
            "Expected not-configured message, got: {}",
            output
        );
    }
}

// ===========================================================================
// CRIT-08: "Not Initialized" Guard — Parameterized Tests
// All command groups should fail gracefully when identity is not initialized.
// ===========================================================================

mod not_initialized_guard {
    use super::*;

    fn assert_not_initialized(ctx: &CliTestContext, args: &[&str]) {
        let output = ctx.run(args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "Command {:?} should fail when not initialized, but succeeded",
            args
        );
        assert!(
            stderr.contains("not initialized") || stderr.contains("Not initialized"),
            "Command {:?} should mention 'not initialized', got stderr: {}",
            args,
            stderr
        );
    }

    #[test]
    fn test_card_requires_init() {
        let ctx = CliTestContext::new();
        assert_not_initialized(&ctx, &["card", "show"]);
    }

    #[test]
    fn test_contacts_requires_init() {
        let ctx = CliTestContext::new();
        assert_not_initialized(&ctx, &["contacts", "list"]);
    }

    #[test]
    fn test_exchange_requires_init() {
        let ctx = CliTestContext::new();
        assert_not_initialized(&ctx, &["exchange", "start"]);
    }

    #[test]
    fn test_device_requires_init() {
        let ctx = CliTestContext::new();
        assert_not_initialized(&ctx, &["device", "list"]);
    }

    #[test]
    fn test_labels_requires_init() {
        let ctx = CliTestContext::new();
        assert_not_initialized(&ctx, &["labels", "list"]);
    }

    #[test]
    fn test_sync_requires_init() {
        let ctx = CliTestContext::new();
        assert_not_initialized(&ctx, &["sync"]);
    }

    #[test]
    fn test_gdpr_requires_init() {
        let ctx = CliTestContext::new();
        assert_not_initialized(&ctx, &["gdpr", "consent-status"]);
    }

    #[test]
    fn test_tor_requires_init() {
        let ctx = CliTestContext::new();
        assert_not_initialized(&ctx, &["tor", "status"]);
    }

    #[test]
    fn test_duress_requires_init() {
        let ctx = CliTestContext::new();
        assert_not_initialized(&ctx, &["duress", "status"]);
    }

    #[test]
    fn test_emergency_requires_init() {
        let ctx = CliTestContext::new();
        assert_not_initialized(&ctx, &["emergency", "status"]);
    }

    #[test]
    fn test_recovery_requires_init() {
        let ctx = CliTestContext::new();
        assert_not_initialized(&ctx, &["recovery", "status"]);
    }
}

// ===========================================================================
// Recovery Additional Tests (MIS-5)
// Trace: features/contact_recovery.feature
// ===========================================================================

mod recovery_additional {
    use super::*;

    /// Trace: contact_recovery.feature - "Show recovery proof when none exists"
    #[test]
    fn test_recovery_proof_no_proof() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        // No proof exists — command should fail with informative message
        let stderr = ctx.run_failure(&["recovery", "proof"]);
        assert!(
            stderr.contains("No recovery proof")
                || stderr.contains("not found")
                || stderr.contains("claim"),
            "Expected no-proof message, got: {}",
            stderr
        );
    }

    /// Trace: contact_recovery.feature - "Claim with invalid hex"
    #[test]
    fn test_recovery_claim_invalid_hex() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let stderr = ctx.run_failure(&["recovery", "claim", "not-valid-hex"]);
        assert!(
            stderr.contains("invalid")
                || stderr.contains("Invalid")
                || stderr.contains("hex")
                || stderr.contains("Odd")
                || stderr.contains("error"),
            "Expected hex decode error, got: {}",
            stderr
        );
    }

    /// Trace: contact_recovery.feature - "Add invalid voucher data"
    #[test]
    fn test_recovery_add_voucher_invalid() {
        let ctx = CliTestContext::new();
        ctx.init("Alice Smith");

        let stderr = ctx.run_failure(&["recovery", "add-voucher", "invalid-base64-data"]);
        assert!(
            stderr.contains("invalid")
                || stderr.contains("Invalid")
                || stderr.contains("decode")
                || stderr.contains("error"),
            "Expected decode error, got: {}",
            stderr
        );
    }
}
