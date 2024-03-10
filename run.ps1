$env:RUST_BACKTRACE = '1'
Start-Process cargo -ArgumentList "run --bin sc-server"
Start-Sleep -Seconds 0.5
Start-Process cargo -ArgumentList "run --bin sc-client 127.0.0.1:8080" -RedirectStandardError client1err.log
Start-Process cargo -ArgumentList "run --bin sc-client 127.0.0.1:8080" -RedirectStandardError client2err.log