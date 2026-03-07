use crate::{Core, CoreEvent};

pub(crate) async fn execute(core: &mut Core, sql: String) -> Vec<CoreEvent> {
    core.base_sql = Some(sql.clone());
    core.effective_sql = Some(sql);
    core.sort_state.clear();
    core.active_filter = None;
    core.execute_current_page(0).await
}

pub(crate) async fn fetch_page(core: &mut Core, page: usize) -> Vec<CoreEvent> {
    core.execute_current_page(page).await
}
