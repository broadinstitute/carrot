create view run_software_versions_with_identifiers
select run_id, software.name as "software", commit as "identifier" from run_software_version inner join software_version using (software_version_id) inner join software using (software_id)
union
select run_id, software.name as "software", tag as "identifier" from run_software_version inner join software_version using (software_version_id) inner join software using (software_id) inner join software_version_tag using (software_version_id);