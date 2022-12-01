drop view if exists software_version_with_tags;

drop table if exists software_version_tag;

alter table software_version
    drop column if exists commit_date;