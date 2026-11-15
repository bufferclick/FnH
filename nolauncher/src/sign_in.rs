//! The Microsoft sign-in, in the player's own browser.
//!
//! This is the launcher's sign-in exactly — the OAuth device code, Xbox Live, XSTS, the Minecraft
//! services and the ownership check (docs/microsoft-account.md), all of it in `horizon-launcher`'s
//! `account` module, which keeps the refresh token in the OS keychain. Only the way the code
//! reaches the player differs: there is no window to print it in, so it goes on the clipboard and
//! on the page, and the browser opens on Microsoft's own sign-in.
//!
//! Nothing here needs an Azure registration of your own: the code comes from the public client
//! Horizon's builds already sign in with, so "Microsoft account" is the whole requirement.

use crate::host;
use crate::page::Page;
use horizon_launcher::account::{self, SignIn, SignInState};
use std::time::Duration;

/// How often the background sign-in is looked at.
const POLL: Duration = Duration::from_millis(200);

/// Makes sure an account on this computer owns Minecraft: Java Edition, signing in when none
/// does: the browser opens, the code waits on the clipboard and on the page, and this returns
/// once the account is saved. A computer that signed in already asks nothing.
pub fn ensure(page: &Page, log: &host::Log) -> Result<(), String> {
    if account::client_id().is_none() {
        return Err(account::NOT_CONFIGURED.to_string());
    }
    let accounts = account::Accounts::load();
    if !accounts.accounts.is_empty() && accounts.has_owner(account::store()) {
        log.line("Signed in already.");
        return Ok(());
    }
    log.line("Signing in with Microsoft.");
    page.step(
        "Signing in with Microsoft",
        "Asking Microsoft for a sign-in code…",
        None,
    );
    let sign_in = SignIn::start();
    let mut opened = false;
    loop {
        match sign_in.state() {
            SignInState::Starting => {}
            SignInState::Code(code) => {
                let (user_code, link) = (code.user_code.clone(), code.verification_uri.clone());
                page.sign_in(&user_code, &link);
                if !opened {
                    opened = true;
                    log.line(&format!("Signing in at {link} with the code {user_code}."));
                    if !host::copy(&user_code) {
                        log.line("The code could not be put on the clipboard; it is on the page.");
                    }
                    // The page first, so it is a tab behind Microsoft's, then Microsoft's own
                    // page in front: the player signs in and the tab behind says how far it got.
                    match page.show() {
                        Ok(()) => {
                            if let Err(e) = host::open(&link) {
                                log.line(&format!("{e} Open {link} in a browser yourself."));
                            }
                        }
                        Err(e) => log.line(&format!("{e} Open {link} in a browser yourself.")),
                    }
                }
            }
            SignInState::Working(phase) => {
                page.step("Signing in with Microsoft", phase.label(), None);
            }
            SignInState::Done(Ok(account)) => {
                log.line(&format!("Signed in as {}.", account.name));
                page.step(
                    "Signing in with Microsoft",
                    &format!("Signed in as {}.", account.name),
                    None,
                );
                return Ok(());
            }
            SignInState::Done(Err(e)) => return Err(e),
        }
        std::thread::sleep(POLL);
    }
}
