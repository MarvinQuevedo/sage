 docker run -it `
>>   -p 9257:9257 `
>>   --add-host=host.docker.internal:host-gateway `
>>   -v ${PWD}/wallets:/root/.local/share/com.rigidnetwork.sage/wallets `
>>   -v ${PWD}/peers:/root/.local/share/com.rigidnetwork.sage/peers `
>>   sage-wallet