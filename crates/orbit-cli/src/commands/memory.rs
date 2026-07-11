use anyhow::Result;
use clap::{Args, Subcommand};
use orbit_core::memory;

#[derive(Debug, Args)]
pub struct MemoryArgs {
    #[command(subcommand)]
    pub command: MemoryCommand,
}

#[derive(Debug, Subcommand)]
pub enum MemoryCommand {
    /// Search past plan runs by semantic similarity to a query
    Search {
        /// Query intent to search for
        query: String,
        /// Number of results to return
        #[arg(long, short, default_value = "5")]
        limit: usize,
    },
    /// List recent plan runs
    List {
        /// Number of runs to show
        #[arg(long, short, default_value = "10")]
        limit: usize,
    },
    /// Show a specific plan run record
    Show {
        /// Plan ID
        plan_id: String,
    },
    /// Show aggregate statistics over all memory records
    Stats,
    /// Delete all memory records
    Clear {
        /// Show what would be deleted without deleting
        #[arg(long)]
        dry_run: bool,
    },
}

pub fn run(args: MemoryArgs) -> Result<()> {
    match args.command {
        MemoryCommand::Search { query, limit } => {
            let results = memory::find_similar(&query, limit);
            if results.is_empty() {
                println!("No similar past plans found for: {query}");
                println!("(Run more plans to build up memory context)");
                return Ok(());
            }
            println!("Top {} match(es) for: {query}", results.len());
            println!();
            for (i, r) in results.iter().enumerate() {
                println!(
                    "  {}. [{}] {} — {} node(s), {}s",
                    i + 1,
                    r.outcome,
                    r.plan_id,
                    r.node_count,
                    r.duration_secs,
                );
                println!("     {}", r.intent);
                if !r.scope_key.is_empty() {
                    println!("     scope: {}", r.scope_key);
                }
                println!();
            }
        }

        MemoryCommand::List { limit } => {
            let runs = memory::load_recent_runs(limit);
            if runs.is_empty() {
                println!("No plan runs in memory.");
                println!("Memory is populated automatically when plans complete.");
                return Ok(());
            }
            println!("{} recent plan run(s):", runs.len());
            println!();
            for r in runs.iter().rev() {
                let outcome_icon = if r.outcome == "Completed" { "✓" } else { "✗" };
                println!(
                    "  {} {} — {} node(s), {}s — {}",
                    outcome_icon, r.plan_id, r.node_count, r.duration_secs, r.outcome
                );
                println!("    {}", r.intent);
            }
        }

        MemoryCommand::Show { plan_id } => {
            match memory::find_run(&plan_id) {
                None => {
                    eprintln!("No memory record found for plan: {plan_id}");
                    std::process::exit(1);
                }
                Some(r) => {
                    println!("Plan:     {}", r.plan_id);
                    println!("Intent:   {}", r.intent);
                    println!("Outcome:  {}", r.outcome);
                    println!("Nodes:    {}", r.node_count);
                    println!("Replans:  {}", r.replan_count);
                    println!("Duration: {}s", r.duration_secs);
                    println!("Scope:    {}", r.scope_key);
                    if !r.tags.is_empty() {
                        println!("Tags:     {}", r.tags.join(", "));
                    }
                }
            }
        }

        MemoryCommand::Stats => {
            let s = memory::memory_stats();
            if s.total_runs == 0 {
                println!("No plan runs in memory yet.");
                return Ok(());
            }
            let success_rate = if s.total_runs > 0 {
                s.completed as f64 / s.total_runs as f64 * 100.0
            } else {
                0.0
            };
            println!("Memory stats ({} plan runs):", s.total_runs);
            println!();
            println!(
                "  Success rate:    {:.0}%  ({} completed, {} failed)",
                success_rate, s.completed, s.failed
            );
            println!("  Avg duration:    {:.1}s", s.avg_duration_secs);
            println!("  Avg nodes/plan:  {:.1}", s.avg_node_count);
            println!("  Avg replans:     {:.1}", s.avg_replan_count);
            if !s.top_scopes.is_empty() {
                println!();
                println!("  Top scopes:");
                for (scope, count) in &s.top_scopes {
                    println!("    {} runs  —  {}", count, scope);
                }
            }
            if s.total_cost_usd > 0.0 {
                println!();
                println!("  Total cost:      ${:.4}", s.total_cost_usd);
                println!("  Total tokens:    {}", s.total_tokens);
                if !s.cost_by_scope.is_empty() {
                    println!();
                    println!("  Cost by scope:");
                    for (scope, cost) in &s.cost_by_scope {
                        println!("    ${:.4}  —  {}", cost, scope);
                    }
                }
                if !s.cost_by_template.is_empty() {
                    println!();
                    println!("  Cost by template:");
                    for (tmpl, cost) in &s.cost_by_template {
                        println!("    ${:.4}  —  {}", cost, tmpl);
                    }
                }
            }
        }

        MemoryCommand::Clear { dry_run } => {
            if dry_run {
                let s = memory::memory_stats();
                println!("(dry-run) Would delete {} plan run record(s).", s.total_runs);
            } else {
                match memory::clear_memory() {
                    Ok(0) => println!("Memory is already empty."),
                    Ok(n) => println!("Deleted {n} plan run record(s)."),
                    Err(e) => {
                        eprintln!("Failed to clear memory: {e}");
                        std::process::exit(1);
                    }
                }
            }
        }
    }
    Ok(())
}
