<template>
    <b-jumbotron v-if="show" style="position:relative">
        <template v-slot:header>Welcome to zkSync
            <div class="jumbotronCloseButton" @click="dontshow"><img src="./assets/cross.png" width="20" height="20"></div>
        </template>
        <template v-slot:lead>
        This is a testnet of ZK Rollup for ERC-20 transfers. Play around with the demo!
        </template>

        <b-button class="tryWalletButton" size="lg" variant="primary" href="/client/" target="_blank" rel="noopener noreferrer">Try demo wallet</b-button>
    </b-jumbotron>
</template>

<script>
export default {
    name: 'ClosableJumbotron',
    data: () => ({
        show: true,
    }),
    methods: {
        dontshow() {
            this.show = false;
        },
    },
};
</script>

<style scoped>
.jumbotronCloseButton {
    position: absolute; 
    top: 20px; 
    right: 20px;
    font-size: 20px;
    cursor: pointer;
}
.tryWalletButton {
    font-size: 1.3em;
}
</style>
