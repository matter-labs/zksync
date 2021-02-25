local G = import '../generator.libsonnet';
local metrics = [
    'api.forced_exit_requests.v01.status',
    'api.forced_exit_requests.v01.submit_request',
    'api.forced_exit_requests.v01.get_request_by_id',
    'forced_exit_requests.get_funds_received_events',
    'forced_exit_requests.eth_watcher.enter_backoff_mode',
    'forced_exit_requests.address_space_overflow'
    'sql.forced_exit_requests.store_request',
    'sql.forced_exit_requests.get_request_by_id',
    'sql.forced_exit_requests.set_fulfilled_at',
    'sql.forced_exit_requests.get_oldest_unfulfilled_request'
    'sql.forced_exit_requests.set_fulfilled_by',
    'sql.forced_exit_requests.get_unconfirmed_requests'
];

G.dashboard('forced_exit_requests', metrics)
