/**
 * Example JavaScript client to interact with ChainLearn contracts using Stellar SDK.
 */
import { Keypair, SorobanRpc, Contract, Networks, TransactionBuilder, BASE_FEE, xdr } from 'stellar-sdk';

const RPC_URL = 'https://soroban-testnet.stellar.org:443';
const NETWORK_PASSPHRASE = Networks.TESTNET;
const PROGRESS_TRACKER_ID = 'C...'; // Replace with actual contract ID

export async function enrollLearner(secretKey, courseId) {
    const server = new SorobanRpc.Server(RPC_URL);
    const keypair = Keypair.fromSecret(secretKey);
    const contract = new Contract(PROGRESS_TRACKER_ID);

    // Fetch account details
    const account = await server.getAccount(keypair.publicKey());
    
    // Build transaction
    const tx = new TransactionBuilder(account, {
        fee: BASE_FEE,
        networkPassphrase: NETWORK_PASSPHRASE,
    })
    .addOperation(contract.call('enroll', xdr.ScVal.scvAddress(xdr.ScAddress.scAddressTypeAccount(keypair.xdrPublicKey())), xdr.ScVal.scvSymbol(courseId)))
    .setTimeout(30)
    .build();

    // Prepare, sign and submit
    const preparedTx = await server.prepareTransaction(tx);
    preparedTx.sign(keypair);
    
    const sendResponse = await server.sendTransaction(preparedTx);
    console.log(`Transaction submitted: ${sendResponse.hash}`);
    return sendResponse;
}
