<template>
<div>
    <b-navbar toggleable="md" type="dark" variant="info">
    <b-container>
        <b-navbar-brand>Franklin Block Explorer</b-navbar-brand>
        <b-navbar-brand right>{{apiServer}}</b-navbar-brand>
    </b-container>
    </b-navbar>
    <br>
    <b-container>
        <b-row>
            <b-col sm="6" class="col-xl-6 col-lg-6 col-md-6 col-sm-12">
                <b-card title="Blocks">
                </b-card>
            </b-col>
            <b-col sm="6" class="col-xl-6 col-lg-6 col-md-6 col-sm-12">
                <b-card title="Transactions">
                </b-card>
            </b-col>
        </b-row>
    </b-container>
</div>
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

h1, h2, h3, h4 {
    font-weight: bold;
}

h4 {
    font-size: 1rem;
}
</style>