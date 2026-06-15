use crate::models::data_structure::job::JobState;

pub trait JobSortable {
    fn get_id(&self) -> &u32;
    fn get_owner(&self) -> &str;
    fn get_state(&self) -> &JobState;
    fn get_start_time(&self) -> u64;
    fn get_walltime(&self) -> u64;
    fn get_queue(&self) -> &str;
    fn get_command(&self) -> &str;
    fn get_message(&self) -> Option<&str>;
    fn get_submission_time(&self) -> u64;
    fn get_scheduled_start(&self) -> u64;
    fn get_stop_time(&self) -> u64;
    fn get_exit_code(&self) -> &Option<i32>;
    fn get_clusters(&self) -> &Vec<String>;
    fn get_end_date(&self) -> i64;
}
