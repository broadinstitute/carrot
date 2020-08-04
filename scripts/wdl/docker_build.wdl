version 1.0

task build_and_push {
    input {
        String repo_url
        String software_name
        String commit_hash
        String registry_host
    }
    command {
        mkdir repo-folder
        cd repo-folder
        git clone ${repo_url} .
        git checkout ${commit_hash}
        docker build -t ${software_name}:${commit_hash} .
        docker tag ${software_name}:${commit_hash} ${registry_host}/${software_name}
        docker push ${registry_host}/${software_name}
    }
    runtime {
        docker: "docker:latest"
    }
}

workflow docker_build {

    input {
        String repo_url
        String software_name
        String commit_hash
        String registry_host
    }

    call build_and_push {
        input:
            repo_url = repo_url,
            software_name = software_name,
            commit_hash = commit_hash,
            registry_host = registry_host
    }

}