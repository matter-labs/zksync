use zksync_testkit::scenarios::perform_basic_tests;

#[tokio::main]
async fn main() {
    perform_basic_tests().await;
}
