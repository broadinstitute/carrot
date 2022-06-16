version 1.0

task build_and_push {

    input {
        String repo_url
        String software_name
        String commit_hash
        String registry_host
        String machine_type
    }

    parameter_meta {
        repo_url: "The url of the repository containing the project to build from"
        software_name: "The name that will be used to name the docker image"
        commit_hash: "The hash for the commit to build from; will also be used to tag the image"
        registry_host: "The docker repository to push the image to"
        machine_type: "GCloud machine type to use for the build"
    }

    command {
        mkdir repo-folder
        cd repo-folder
        git clone ${repo_url} .
        git checkout ${commit_hash}
        echo -n "" >> .gcloudignore
        if [ -z ${machine_type} ]
        then
            gcloud builds submit --tag ${registry_host}/${software_name}:${commit_hash} --timeout=24h
        else
            gcloud builds submit --tag ${registry_host}/${software_name}:${commit_hash} --timeout=24h --machine-type ${machine_type}
        fi
    }

    runtime {
        docker: "google/cloud-sdk:307.0.0"
    }

}

workflow docker_build {

    input {
        String repo_url
        String software_name
        String commit_hash
        String registry_host
        String machine_type = ""
    }

    parameter_meta {
        repo_url: "The url of the repository containing the project to build from"
        software_name: "The name that will be used to name the docker image"
        commit_hash: "The hash for the commit to build from; will also be used to tag the image"
        registry_host: "The docker repository to push the image to"
        machine_type: "GCloud machine type to use for the build"
    }

    call build_and_push {
        input:
            repo_url = repo_url,
            software_name = software_name,
            commit_hash = commit_hash,
            registry_host = registry_host,
            machine_type = machine_type
    }

    output {
        String image_url = registry_host + '/' + software_name + ':' + commit_hash
    }

}
