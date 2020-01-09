import config from "./env-config";

export default Object.freeze({
    PAGE_SIZE: 20,
    TX_BATCH_SIZE: config.TX_BATCH_SIZE,
});
