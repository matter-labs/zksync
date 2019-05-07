<template>
<b-container id="login">
    <b-jumbotron bg-variant="light" border-variant="dark">
        <template slot="header">
            Matter Network Wallet <span style="font-size: 0.3em"><strong>ALPHA</strong></span>
        </template>
        <template slot="lead">
            <span class="local">API at {{apiServer}}</span>
        </template>
    </b-jumbotron>
</b-container>
</template>

<script>

import store from './store'

export default {
    name: 'home',
    methods: {
    },
}
</script>

<style>

h1, h2 {
    font-weight: normal;
}

</style>