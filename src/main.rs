/**
Copyright (C) 2013, 2021 Kunal Mehta <legoktm@member.fsf.org>

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */
use anyhow::{anyhow, Result};
use chrono::{prelude::*, Duration};
use log::{debug, error, info};
use mwapi_responses::{prelude::*, protection::ProtectionInfo};
use mwbot::{Bot, Error};

#[query(prop = "info", inprop = "protection")]
struct ProtectionResponse;

/// Query the current protection status of a page
async fn protection_status(
    name: &str,
    bot: &Bot,
) -> Result<Vec<ProtectionInfo>> {
    let mut params = ProtectionResponse::params().to_vec();
    params.push(("titles", name));
    let resp: ProtectionResponse = bot.get_api().get(&params).await?;
    Ok(resp.query.pages[0].protection.clone())
}

async fn handle_page(
    name: &str,
    day: Date<Utc>,
    bot: &Bot,
    // Whether we also need to apply edit protection
    redirect: bool,
) -> Result<()> {
    let status = protection_status(name, bot).await?;
    // dbg!(&status);
    // let title = Title::new_from_full(name, api);
    // Protect for a day
    let until = day + Duration::days(1);
    let mut move_protected = false;
    let mut edit_protected = false;
    for prot in status.iter() {
        if prot.level == "sysop" {
            if prot.type_ == "move" {
                if prot.expiry == "infinity" {
                    move_protected = true;
                } else {
                    let expiry: DateTime<Utc> = prot.expiry.parse()?;
                    if expiry.date() >= until {
                        move_protected = true;
                    }
                }
            } else if prot.type_ == "edit" {
                if prot.expiry == "infinity" {
                    edit_protected = true;
                } else {
                    let expiry: DateTime<Utc> = prot.expiry.parse()?;
                    if expiry.date() >= until {
                        edit_protected = true;
                    }
                }
            }
        }
    }
    if move_protected && (!redirect || edit_protected) {
        info!("{} is already protected", name);
        return Ok(());
    }
    info!("{} needs to be protected!", name);

    let mut protections = vec![];
    let mut expiry = vec![];
    let mut cascade = false;
    if !move_protected {
        protections.push("move=sysop".to_string());
        expiry.push(
            until
                .and_hms(0, 0, 0)
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string(),
        )
    }
    if redirect && !edit_protected {
        protections.push("edit=sysop".to_string());
        expiry.push(
            until
                .and_hms(0, 0, 0)
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string(),
        )
    }
    for prot in status.iter() {
        if prot.type_ == "aft" {
            // bug 57389
            continue;
        }
        if prot.source.is_some() {
            // skip cascading protection
            continue;
        }
        if (prot.type_ == "move" && !move_protected)
            || (prot.type_ == "edit" && redirect && !edit_protected)
        {
            // don't try to protect what we're changing
            continue;
        }
        if prot.cascade {
            // send it back i guess?
            cascade = true;
        }
        protections.push(format!("{}={}", prot.type_, prot.level));
        expiry.push(prot.expiry.clone());
    }

    let protections = protections.join("|");
    let expiry = expiry.join("|");
    let mut params = vec![
        ("action", "protect"),
        ("title", name),
        ("protections", &protections),
        ("expiry", &expiry),
        ("reason", "Upcoming TFA ([[WP:BOT|bot protection]])"),
    ];
    if cascade {
        params.push(("cascade", "1"));
    }
    bot.get_api().post_with_token("csrf", &params).await?;
    info!("Successfully protected {}", &name);
    Ok(())
}

async fn get_tfa_title(day: Date<Utc>, bot: &Bot) -> Result<String> {
    // First see if we can get it from Template:TFA title
    let tfa_page = bot.get_page(&format!(
        "Template:TFA title/{}",
        day.format("%B %-d, %Y").to_string()
    ));
    match tfa_page.get_wikitext().await {
        Ok(text) => return Ok(text),
        // Nothing, keep trying
        Err(Error::PageDoesNotExist(_)) => {
            debug!("{} didn't exist, will check TFA now", tfa_page.title())
        }
        Err(e) => {
            return Err(anyhow!(e.to_string()));
        }
    };

    // Try harder, parse it out of the TFA page itself
    extract_tfa_title(day, bot).await
}

async fn extract_tfa_title(day: Date<Utc>, bot: &Bot) -> Result<String> {
    use mwbot::parsoid::*;

    let page = bot.get_page(&format!(
        "Wikipedia:Today's featured article/{}",
        day.format("%B %-d, %Y").to_string()
    ));
    let code = page.get_html().await?;
    // Return the first bolded link
    for bold in code.select("b").iter() {
        // unwrap: We know that <b> tags turn into generic nodes
        let links = bold.as_generic().unwrap().filter_links();
        if !links.is_empty() {
            return Ok(links[0].target());
        }
    }
    Err(anyhow!("could not find title for {}", page.title()))
}

#[tokio::main]
async fn main() {
    use flexi_logger::{
        opt_format, Cleanup, Criterion, Duplicate, Logger, Naming,
    };
    let logger = Logger::with_str("info, tfa_protector_bot=debug")
        .log_to_file()
        .duplicate_to_stdout(Duplicate::Info)
        .format(opt_format)
        .suppress_timestamp()
        .append()
        .use_buffering(true)
        .rotate(
            Criterion::Size(5_000_000),
            Naming::Timestamps,
            Cleanup::KeepLogFiles(30),
        )
        .start()
        .unwrap();
    match run().await {
        Ok(_) => info!("Finished successfully"),
        Err(e) => error!("Error: {}", e.to_string()),
    };
    logger.shutdown();
}

async fn run() -> Result<()> {
    let now = Utc::today();
    let bot = Bot::from_default_config().await?;
    for ahead in 1..35 {
        let day = now + Duration::days(ahead);
        let text = match get_tfa_title(day, &bot).await {
            Ok(text) => text,
            Err(e) => {
                debug!("{}", e.to_string());
                info!(
                    "{} is missing, skipping",
                    day.format("%B %-d, %Y").to_string()
                );
                continue;
            }
        };
        let page = bot.get_page(&text);
        match page.get_redirect_target().await? {
            Some(target) => {
                info!("{} redirects to {}", &text, &target.title());
                handle_page(&text, day, &bot, true).await?;
                handle_page(target.title(), day, &bot, false).await?;
            }
            None => {
                handle_page(&text, day, &bot, false).await?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    async fn test_bot() -> Bot {
        Bot::from_path(Path::new("mwbot-test.toml")).await.unwrap()
    }

    #[tokio::test]
    async fn test_protection_status() {
        let bot = test_bot().await;
        assert!(protection_status("User:Legoktm/test", &bot)
            .await
            .unwrap()
            .is_empty());
        let einstein =
            protection_status("Albert Einstein", &bot).await.unwrap();
        assert_eq!(einstein[0].type_, "edit".to_string());
        assert_eq!(einstein[0].level, "autoconfirmed".to_string());
        assert_eq!(einstein[0].expiry, "infinity".to_string());
        assert_eq!(einstein[1].type_, "move".to_string());
        assert_eq!(einstein[1].level, "sysop".to_string());
        assert_eq!(einstein[1].expiry, "infinity".to_string());
    }

    #[tokio::test]
    async fn test_extract_tfa_title() {
        let bot = test_bot().await;
        assert_eq!(
            "Zoo TV Tour".to_string(),
            extract_tfa_title(Utc.ymd(2020, 02, 29), &bot)
                .await
                .unwrap()
        );
        // Case normalization
        assert_eq!(
            "Mosaics of Delos".to_string(),
            extract_tfa_title(Utc.ymd(2020, 02, 5), &bot).await.unwrap()
        );
        // Unicode (was broken in Python)
        assert_eq!(
            "SMS Zähringen".to_string(),
            extract_tfa_title(Utc.ymd(2020, 02, 6), &bot).await.unwrap()
        );
        // Italics
        assert_eq!(
            "The Cabinet of Dr. Caligari".to_string(),
            extract_tfa_title(Utc.ymd(2020, 02, 26), &bot)
                .await
                .unwrap()
        );
    }
}
