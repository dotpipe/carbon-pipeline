import {
    http {
        source="target/release/libhttp_server"
    }
    switch {
        source="target/release/libswitch"
    }
    payload {
        source="target/release/libpayload"
    }
    request {
        source="target/release/librequest"
    }
}

pipeline {
   http "init" {
    producer = true
    path "/send/{number}" {
        attach payload { // TODO Importa dinamicamente o module e associa ao attach
            params {
                body = ${ params.number }
            }
        }
        status_code=200
    }

    path "/health" {s
        attach= "health"
        default_status_code=202
    }
  }

   switch "cases" {
    case {
        params {
            operator = eq
            left = ${ params.number }
            rigth = 3
        }
    }

    case {
        conditional = "eq params.number other"
        attach = "number"
    }
  }


    script "db" {
        runtime=nodejs
        path="/handler/index.js"
        connector=rpc
        args {

        }
    }
}
