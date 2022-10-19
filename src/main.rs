use run::run;

mod run;
mod state;

fn main() {
    pollster::block_on(run());
}
