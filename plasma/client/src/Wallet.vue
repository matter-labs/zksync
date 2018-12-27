<template>
    <b-jumbotron bg-variant="light" border-variant="dark">
    <template slot="header">
        Plasma Wallet
    </template>
    <hr class="my-4">
        {{ store.account }}
    </b-jumbotron>
</template>

<script>

import store from './store'

export default {
    name: 'wallet',
    data: () => ({ store }),
    methods: {
    },
}
</script>
