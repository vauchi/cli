// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Display Helpers
//!
//! Terminal output formatting and styling.

#![allow(dead_code)] // Utility functions for future use

use console::{style, Style};
use tabled::{
    settings::{object::Columns, Alignment, Modify, Style as TableStyle},
    Table, Tabled,
};
use vauchi_core::{Contact, ContactCard, FieldType, SocialNetworkRegistry};

/// Prints a success message.
pub fn success(msg: &str) {
    println!("{} {}", style("✓").green().bold(), msg);
}

/// Prints an error message.
pub fn error(msg: &str) {
    eprintln!("{} {}", style("✗").red().bold(), msg);
}

/// Prints a warning message.
pub fn warning(msg: &str) {
    println!("{} {}", style("⚠").yellow().bold(), msg);
}

/// Prints an info message.
pub fn info(msg: &str) {
    println!("{} {}", style("ℹ").blue().bold(), msg);
}

/// Returns the icon for a field type.
fn field_icon(field_type: FieldType) -> &'static str {
    match field_type {
        FieldType::Email => "mail",
        FieldType::Phone => "phone",
        FieldType::Website => "web",
        FieldType::Address => "home",
        FieldType::Social => "share",
        FieldType::Custom => "note",
    }
}

/// Displays a contact card in a formatted box.
pub fn display_card(card: &ContactCard) {
    let name = card.display_name();
    let width = 50;
    let registry = SocialNetworkRegistry::with_defaults();

    // Top border
    println!("{}", "─".repeat(width));

    // Name
    println!("  {}", style(name).bold().cyan());

    // Separator
    println!("{}", "─".repeat(width));

    // Fields
    if card.fields().is_empty() {
        println!("  {}", style("(no fields)").dim());
    } else {
        for field in card.fields() {
            let icon = field_icon(field.field_type());
            let label_style = Style::new().dim();

            // For social fields, try to generate profile URL
            if field.field_type() == FieldType::Social {
                let label_lower = field.label().to_lowercase();
                if let Some(url) = registry.profile_url(&label_lower, field.value()) {
                    println!(
                        "  {:6} {:12} {}",
                        icon,
                        label_style.apply_to(field.label()),
                        field.value()
                    );
                    println!("         {:12} {}", "", style(&url).dim().underlined());
                } else {
                    println!(
                        "  {:6} {:12} {}",
                        icon,
                        label_style.apply_to(field.label()),
                        field.value()
                    );
                }
            } else {
                println!(
                    "  {:6} {:12} {}",
                    icon,
                    label_style.apply_to(field.label()),
                    field.value()
                );
            }
        }
    }

    // Bottom border
    println!("{}", "─".repeat(width));
}

/// Displays a contact in a compact format.
pub fn display_contact_summary(contact: &Contact, index: usize) {
    let name = contact.display_name();
    let verified = if contact.is_fingerprint_verified() {
        style("✓ verified").green()
    } else {
        style("").dim()
    };

    println!("  {}. {}  {}", index, style(name).bold(), verified);
}

/// Displays a contact with full details.
pub fn display_contact_details(contact: &Contact) {
    let name = contact.display_name();
    let id = contact.id();

    println!();
    println!("  {}", style(name).bold().cyan());
    println!("  ID: {}", style(id).dim());

    if contact.is_fingerprint_verified() {
        println!("  Status: {}", style("Fingerprint verified").green());
    } else {
        println!("  Status: {}", style("Not verified").yellow());
    }

    if contact.is_recovery_trusted() {
        println!("  Recovery: {}", style("Trusted").green());
    }

    println!();

    // Show card fields
    let card = contact.card();
    if card.fields().is_empty() {
        println!("  {}", style("(no visible fields)").dim());
    } else {
        for field in card.fields() {
            let icon = field_icon(field.field_type());
            println!(
                "  {:6} {:12} {}",
                icon,
                style(field.label()).dim(),
                field.value()
            );
        }
    }

    println!();
}

/// Displays a QR code in the terminal using Unicode blocks.
pub fn display_qr_code(data: &str) {
    use qrcode::render::unicode;
    use qrcode::QrCode;

    match QrCode::new(data) {
        Ok(code) => {
            let image = code
                .render::<unicode::Dense1x2>()
                .dark_color(unicode::Dense1x2::Light)
                .light_color(unicode::Dense1x2::Dark)
                .build();
            println!("{}", image);
        }
        Err(e) => {
            error(&format!("Failed to generate QR code: {}", e));
        }
    }
}

/// Displays exchange data for sharing.
pub fn display_exchange_data(data: &str) {
    println!();
    println!("Scan this QR code with another Vauchi user:");
    println!();
    display_qr_code(data);
    println!();
    println!("Or share this text:");
    println!("{}", style(data).cyan());
    println!();
}

/// Displays the list of available social networks.
pub fn display_social_networks(query: Option<&str>) {
    let registry = SocialNetworkRegistry::with_defaults();

    let networks: Vec<_> = if let Some(q) = query {
        registry.search(q)
    } else {
        registry.all()
    };

    if networks.is_empty() {
        if let Some(q) = query {
            println!("No social networks matching '{}'", q);
        } else {
            println!("No social networks available");
        }
        return;
    }

    println!();
    println!("{}", style("Available Social Networks").bold());
    println!("{}", "─".repeat(50));
    println!();

    // Group by category
    let mut printed = 0;
    for network in &networks {
        println!(
            "  {:16} {}",
            style(network.id()).cyan(),
            network.display_name()
        );
        println!(
            "  {:16} {}",
            "",
            style(network.profile_url_template()).dim()
        );
        printed += 1;
        if printed % 5 == 0 {
            println!();
        }
    }

    println!();
    println!("{}", "─".repeat(50));
    println!(
        "Use: {} {} {}",
        style("vauchi card add social").cyan(),
        style("<network>").yellow(),
        style("<username>").yellow()
    );
    println!(
        "Example: {}",
        style("vauchi card add social github octocat").dim()
    );
    println!();
}

/// Row structure for contact table display.
#[derive(Tabled)]
struct ContactRow {
    #[tabled(rename = "#")]
    index: usize,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Recovery")]
    recovery: String,
}

/// Displays a list of contacts as a formatted table.
pub fn display_contacts_table(contacts: &[Contact]) {
    let rows: Vec<ContactRow> = contacts
        .iter()
        .enumerate()
        .map(|(i, c)| ContactRow {
            index: i + 1,
            name: c.display_name().to_string(),
            id: format!("{}...", &c.id()[..8.min(c.id().len())]),
            status: if c.is_fingerprint_verified() {
                "✓ verified".to_string()
            } else {
                "not verified".to_string()
            },
            recovery: if c.is_recovery_trusted() {
                "★".to_string()
            } else {
                String::new()
            },
        })
        .collect();

    let table = Table::new(rows)
        .with(TableStyle::rounded())
        .with(Modify::new(Columns::first()).with(Alignment::right()))
        .to_string();

    println!("{}", table);
}

// ============================================================
// FAQ Display Functions
// ============================================================

use vauchi_core::help::{get_faqs, get_faqs_by_category, search_faqs, HelpCategory};
use vauchi_core::i18n::{get_string, Locale};

/// Parse locale code to Locale enum
fn parse_locale(code: &str) -> Locale {
    Locale::from_code(code).unwrap_or(Locale::English)
}

/// Get localized string
fn t(key: &str, locale: &str) -> String {
    get_string(parse_locale(locale), key)
}

/// Displays FAQ items, optionally filtered by search query.
pub fn display_faqs(query: Option<&str>, locale: &str) {
    let faqs = if let Some(q) = query {
        search_faqs(q)
    } else {
        get_faqs()
    };

    if faqs.is_empty() {
        if let Some(q) = query {
            println!("No FAQs matching '{}'", q);
        } else {
            println!("No FAQs available");
        }
        return;
    }

    println!();
    let title = if let Some(q) = query {
        format!("{} ({})", t("help.faq", locale), q)
    } else {
        t("help.faq", locale)
    };
    println!("{}", style(title).bold());
    println!("{}", "─".repeat(60));
    println!();

    for faq in faqs {
        println!("{}", style(&faq.question).cyan().bold());
        // Word wrap the answer at 60 chars
        for line in wrap_text(&faq.answer, 60) {
            println!("  {}", line);
        }
        println!();
    }
}

/// Displays FAQ categories.
pub fn display_faq_categories(locale: &str) {
    println!();
    println!("{}", style(t("help.faq", locale)).bold());
    println!("{}", "─".repeat(40));
    println!();

    let categories = [
        ("getting-started", HelpCategory::GettingStarted),
        ("privacy", HelpCategory::Privacy),
        ("recovery", HelpCategory::Recovery),
        ("contacts", HelpCategory::Contacts),
        ("updates", HelpCategory::Updates),
        ("features", HelpCategory::Features),
    ];

    for (id, category) in &categories {
        let faqs = get_faqs_by_category(*category);
        println!(
            "  {:16} {} ({} FAQs)",
            style(id).cyan(),
            category.display_name(),
            faqs.len()
        );
    }

    println!();
    println!("{}", "─".repeat(40));
    println!("Use: {}", style("vauchi help category <name>").cyan());
    println!();
}

/// Displays FAQs for a specific category.
pub fn display_faqs_by_category(category_name: &str, locale: &str) {
    let category = match category_name.to_lowercase().as_str() {
        "getting-started" | "gettingstarted" | "start" => Some(HelpCategory::GettingStarted),
        "privacy" | "security" => Some(HelpCategory::Privacy),
        "recovery" => Some(HelpCategory::Recovery),
        "contacts" | "contact" => Some(HelpCategory::Contacts),
        "updates" | "sync" => Some(HelpCategory::Updates),
        "features" | "feature" => Some(HelpCategory::Features),
        _ => None,
    };

    let Some(cat) = category else {
        error(&format!("Unknown category: {}", category_name));
        info("Valid categories: getting-started, privacy, recovery, contacts, updates, features");
        return;
    };

    let faqs = get_faqs_by_category(cat);

    if faqs.is_empty() {
        println!("No FAQs in category '{}'", category_name);
        return;
    }

    println!();
    println!(
        "{}: {}",
        style(t("help.faq", locale)).bold(),
        style(cat.display_name()).cyan()
    );
    println!("{}", "─".repeat(60));
    println!();

    for faq in faqs {
        println!("{}", style(&faq.question).cyan().bold());
        for line in wrap_text(&faq.answer, 60) {
            println!("  {}", line);
        }
        println!();
    }
}

/// Displays a specific FAQ by ID.
pub fn display_faq_by_id(id: &str, locale: &str) {
    use vauchi_core::help::get_faq_by_id;

    match get_faq_by_id(id) {
        Some(faq) => {
            println!();
            println!(
                "{}: {}",
                style(t("help.faq", locale)).bold(),
                style(&faq.id).dim()
            );
            println!("{}", "─".repeat(60));
            println!();
            println!("{}", style(&faq.question).cyan().bold());
            for line in wrap_text(&faq.answer, 60) {
                println!("  {}", line);
            }
            if !faq.related.is_empty() {
                println!();
                println!("  Related: {}", faq.related.join(", "));
            }
            println!();
        }
        None => {
            error(&format!("FAQ not found: {}", id));
            info("Use 'vauchi faq list' to see available FAQs");
        }
    }
}

// ============================================================
// Aha Moment Display
// ============================================================

use vauchi_core::aha_moments::AhaMoment;

/// Displays an aha moment as a highlighted info box.
pub fn display_aha_moment(moment: &AhaMoment) {
    let border = "─".repeat(50);
    let top = format!("┌{}┐", border);
    let bottom = format!("└{}┘", border);

    println!();
    println!("{}", style(&top).magenta());
    println!(
        "│ {} {}{}│",
        style("★").magenta().bold(),
        style(moment.title()).magenta().bold(),
        " ".repeat(50 - 3 - moment.title().len())
    );
    println!("│{}│", " ".repeat(50));
    for line in wrap_text(&moment.message(), 46) {
        let padding = 48 - line.len();
        println!("│  {}{}│", line, " ".repeat(padding));
    }
    println!("{}", style(&bottom).magenta());
    println!();
}

/// Simple text wrapping.
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();

    for paragraph in text.lines() {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        let words: Vec<&str> = paragraph.split_whitespace().collect();
        let mut current_line = String::new();

        for word in words {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= max_width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }

    lines
}
