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
use chrono::prelude::*;
use chrono::Duration;
use mediawiki::{
    api::Api,
    page::{Page, PageError},
    title::Title,
};
use serde::Deserialize;
use std::collections::HashMap;

const USER_AGENT: &str = toolforge::user_agent!("tfaprotbot");

/// Login information, stored in auth.toml
#[derive(Deserialize)]
struct Auth {
    username: String,
    password: String,
}

/// Get an `Api` instance
async fn mwapi() -> Result<Api> {
    let mut api = Api::new("https://en.wikipedia.org/w/api.php").await?;
    api.set_user_agent(USER_AGENT);
    api.set_maxlag(Some(999999999)); // Don't worry about it
    let path = std::path::Path::new("auth.toml");
    if path.exists() {
        let auth: Auth = toml::from_str(&std::fs::read_to_string(path)?)?;
        println!("Logging in as {}", auth.username);
        api.login(auth.username, auth.password).await?;
    }
    Ok(api)
}

/// A protection entry
#[derive(Deserialize, Debug)]
struct Protection {
    #[serde(rename = "type")]
    type_: String,
    level: String,
    expiry: String,
    #[serde(default)]
    cascade: bool,
    /// Source the cascade protection is coming from
    source: Option<String>,
}

/// Query the current protection status of a page
async fn protection_status(name: &str, api: &Api) -> Result<Vec<Protection>> {
    let params = [
        ("action", "query"),
        ("prop", "info"),
        ("inprop", "protection"),
        ("titles", name),
        ("formatversion", "2"),
    ]
    .iter()
    .map(|&(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let result = api.get_query_api_json(&params).await?;
    let status: Vec<Protection> = serde_json::from_value(
        result["query"]["pages"][0]["protection"].clone(),
    )?;
    Ok(status)
}

/// Get the page this one redirects to, if it's a redirect
/// TODO: upstream this to mediawiki crate
async fn get_redirect_target(name: &str, api: &Api) -> Result<Option<String>> {
    let params = [
        ("action", "query"),
        ("redirects", "1"),
        ("titles", name),
        ("formatversion", "2"),
    ]
    .iter()
    .map(|&(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let result = api.get_query_api_json(&params).await?;
    match result["query"].get("redirects") {
        Some(redirects) => {
            Ok(redirects[0]["to"].as_str().map(|s| s.to_string()))
        }
        // Not a redirect
        None => Ok(None),
    }
}

async fn handle_page(
    name: &str,
    day: Date<Utc>,
    api: &mut Api,
    // Whether we also need to apply edit protection
    redirect: bool,
) -> Result<()> {
    let status = protection_status(name, api).await?;
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
        println!("{} is already protected", name);
        return Ok(());
    }
    println!("{} needs to be protected!", name);

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

    let mut params: HashMap<String, String> = [
        ("action", "protect"),
        ("title", &name),
        ("protections", &protections.join("|")),
        ("expiry", &expiry.join("|")),
        ("reason", "Upcoming TFA ([[WP:BOT|bot protection]])"),
        ("formatversion", "2"),
        ("token", &api.get_edit_token().await?),
    ]
    .iter()
    .map(|&(k, v)| (k.to_string(), v.to_string()))
    .collect();
    if cascade {
        params.insert("cascade".to_string(), "1".to_string());
    }
    let result = api.post_query_api_json(&params).await?;
    match result.get("error") {
        Some(errors) => Err(anyhow!(serde_json::to_string(errors)?)),
        None => {
            println!("Successfully protected {}", &name);
            Ok(())
        }
    }
}

async fn get_tfa_title(day: Date<Utc>, api: &Api) -> Result<String> {
    // First see if we can get it from Template:TFA title
    let tfa_title = Title::new(
        &format!("TFA title/{}", day.format("%B %-d, %Y").to_string()),
        10,
    );
    let tfa_page = Page::new(tfa_title);
    match tfa_page.text(&api).await {
        Ok(text) => return Ok(text),
        // Nothing, keep trying
        Err(PageError::Missing(title)) => {
            println!(
                "{} didn't exist, will check TFA now",
                title.full_pretty(&api).unwrap()
            )
        }
        Err(e) => {
            return Err(anyhow!(e.to_string()));
        }
    };

    // Try harder, parse it out of the TFA page itself
    extract_tfa_title(day).await
}

async fn extract_tfa_title(day: Date<Utc>) -> Result<String> {
    use parsoid::prelude::*;

    let page = format!(
        "Wikipedia:Today's featured article/{}",
        day.format("%B %-d, %Y").to_string()
    );
    let client =
        Client::new("https://en.wikipedia.org/api/rest_v1", USER_AGENT)?;
    let code = client.get(&page).await?;
    // Return the first bolded link
    for bold in code.select("b").iter() {
        // unwrap: We know that <b> tags turn into generic nodes
        let links = bold.as_generic().unwrap().filter_links();
        if !links.is_empty() {
            return Ok(links[0].pretty_target());
        }
    }
    Err(anyhow!("could not find title for {}", page))
}

#[tokio::main]
async fn main() -> Result<()> {
    let now = Utc::today();
    let mut api = mwapi().await?;
    for ahead in 1..35 {
        let day = now + Duration::days(ahead);
        let text = match get_tfa_title(day, &api).await {
            Ok(text) => text,
            Err(e) => {
                println!("{}", e.to_string());
                println!(
                    "{} is missing, skipping",
                    day.format("%B %-d, %Y").to_string()
                );
                continue;
            }
        };
        match get_redirect_target(&text, &api).await? {
            Some(target) => {
                println!("{} redirects to {}", &text, &target);
                handle_page(&text, day, &mut api, true).await?;
                handle_page(&target, day, &mut api, false).await?;
            }
            None => {
                handle_page(&text, day, &mut api, false).await?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_redirect_target() {
        let api = mwapi().await.unwrap();
        assert_eq!(
            Some("Main Page".to_string()),
            get_redirect_target("Main page", &api).await.unwrap()
        );
        assert_eq!(None, get_redirect_target("Main Page", &api).await.unwrap());
    }

    #[tokio::test]
    async fn test_protection_status() {
        let api = mwapi().await.unwrap();
        assert!(protection_status("User:Legoktm/test", &api)
            .await
            .unwrap()
            .is_empty());
        let einstein =
            protection_status("Albert Einstein", &api).await.unwrap();
        assert_eq!(einstein[0].type_, "edit".to_string());
        assert_eq!(einstein[0].level, "autoconfirmed".to_string());
        assert_eq!(einstein[0].expiry, "infinity".to_string());
        assert_eq!(einstein[1].type_, "move".to_string());
        assert_eq!(einstein[1].level, "sysop".to_string());
        assert_eq!(einstein[1].expiry, "infinity".to_string());
    }

    #[tokio::test]
    async fn test_extract_tfa_title() {
        assert_eq!(
            "Zoo_TV_Tour".to_string(),
            extract_tfa_title(Utc.ymd(2020, 02, 29)).await.unwrap()
        );
        // Case normalization
        assert_eq!(
            "Mosaics_of_Delos".to_string(),
            extract_tfa_title(Utc.ymd(2020, 02, 5)).await.unwrap()
        );
        // Unicode (was broken in Python)
        assert_eq!(
            "SMS_Zähringen".to_string(),
            extract_tfa_title(Utc.ymd(2020, 02, 6)).await.unwrap()
        );
        // Italics
        assert_eq!(
            "The_Cabinet_of_Dr._Caligari".to_string(),
            extract_tfa_title(Utc.ymd(2020, 02, 26)).await.unwrap()
        );
    }
}
