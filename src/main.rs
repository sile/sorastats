use clap::Parser;

#[derive(Debug, Parser)]
#[clap(version)]
struct Args {
    sora_url: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let x: serde_json::Value = ureq::post(&args.sora_url)
        .set("x-sora-target", "Sora_20171101.GetStatsAllConnections")
        .call()?
        .into_json()?;
    println!("{}", x);
    Ok(())
}
