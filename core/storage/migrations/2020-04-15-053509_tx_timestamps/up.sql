alter table executed_priority_operations 
add column created_at timestamp not null default now();
