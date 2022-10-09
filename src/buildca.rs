use log::{debug, info};
use std::process::Stdio;
use thiserror::Error;
use tokio::process::Command;

#[derive(Error, Debug)]
pub enum BuildCAProfileError {
    #[error("Cannot build a content-addressed derivation without a flake.")]
    CADerivationNonFlake,
    #[error("Failed to start Nix build command: {0}")]
    BuildErrorStart(std::io::Error),
    #[error("Nix build command finished with error: {0}")]
    BuildErrorRun(std::io::Error),
    #[error("Nix build command finished with errorcode: {0:?}")]
    BuildErrorCode(Option<i32>),
}

pub struct BuildCAProfileData<'a> {
    pub supports_flakes: bool,
    pub repo: &'a str,
    pub deploy_data: &'a super::DeployData<'a>,
    pub extra_build_args: &'a [String],
}

pub struct CaData {
    pub is_ca: bool,
    pub path: String, //the actual build path
}

pub async fn build_ca_profile(data: BuildCAProfileData<'_>) -> Result<String, BuildCAProfileError> {
    // This function will just check for a CA derivation and evaluate (=build) it
    let mut local_ca_data = CaData {
        is_ca: false,
        path: String::from(""),
    };
    // we are not in a store path. Most likely we try to build a CA derivation
    info!(
            "The path {} does not start with \"nix/store\", so we will assume this is a content-addressed derivation",
            &data.deploy_data.profile.profile_settings.path
        );
    if !data.supports_flakes {
        return Err(BuildCAProfileError::CADerivationNonFlake);
    };
    local_ca_data.is_ca = true;
    let mut build_command = Command::new("nix");

    //TODO: Is it always ".deploy"?
    build_command.arg("build").arg(
        data.repo.to_string()
            + "#deploy.nodes."
            + data.deploy_data.node_name
            + ".profiles."
            + data.deploy_data.profile_name
            + ".path",
    );
    build_command.arg("--no-link");
    for extra_arg in data.extra_build_args {
        build_command.arg(extra_arg);
    }
    debug!("Trying to catch output path after build of the CA derivation",);
    // since this is a CA derivation, the original path is invalid
    // we need to run "nix build" to return the actual path
    build_command.arg("--print-out-paths");

    info!(
        "Bulding CA profile `{}` for node `{}`",
        data.deploy_data.profile_name, data.deploy_data.node_name
    );

    let build_child = build_command
        .stdout(Stdio::piped())
        .spawn()
        .map_err(BuildCAProfileError::BuildErrorStart)?;

    let build_output = build_child
        .wait_with_output()
        .await
        .map_err(BuildCAProfileError::BuildErrorRun)?;

    match build_output.status.code() {
        Some(0) => (),
        a => return Err(BuildCAProfileError::BuildErrorCode(a)),
    };

    let ca_path = String::from_utf8(build_output.stdout)
        .unwrap()
        .trim()
        .to_string();
    local_ca_data.path = ca_path;
    debug!("Actual output path is {}", local_ca_data.path);

    return Ok(local_ca_data.path.to_string());
}
