import { PublicKey, Connection, LAMPORTS_PER_SOL } from '@solana/web3.js'

export async function safeAirdrop(address : PublicKey, connection : Connection){
    //get the address account info using the provider connection
    const accountInfo = await connection.getAccountInfo(address,"confirmed");

    if(accountInfo == null || accountInfo.lamports < LAMPORTS_PER_SOL){
        let tx = await connection.requestAirdrop(
            address,
            LAMPORTS_PER_SOL
        );

        //confirm the airdrop transaction
        await connection.confirmTransaction(tx);
    }

}