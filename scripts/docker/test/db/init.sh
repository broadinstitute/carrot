psql -v ON_ERROR_STOP=1 --username "postgres" -c 'create database test_framework_db;';
psql -v ON_ERROR_STOP=1 --username "postgres" -c "create user test_framework_user with password 'test';";
psql -v ON_ERROR_STOP=1 --username "postgres" -c 'create extension if not exists "uuid-ossp";' test_framework_db;