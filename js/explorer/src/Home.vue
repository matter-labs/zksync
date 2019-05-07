<template>
<div>
    <b-navbar toggleable="md" type="dark" variant="info">
    <b-container>
        <b-navbar-brand>Franklin Block Explorer</b-navbar-brand>
        <b-navbar-brand right>API server: {{apiServer}}</b-navbar-brand>
    </b-container>
    </b-navbar>
    <br>
    <b-container>
        <b-row>
            <b-col sm="6" class="col-xl-6 col-lg-6 col-md-6 col-sm-12">
                  <b-card no-body header="<b>Blocks</b>">
                    <b-list-group flush>
                    <b-list-group-item href="#">Cras justo odio</b-list-group-item>
                    <b-list-group-item href="#">Dapibus ac facilisis in</b-list-group-item>
                    <b-list-group-item href="#">Vestibulum at eros</b-list-group-item>
                    </b-list-group>

                    <b-card-body>
                    <b-button block variant="outline-primary">View more</b-button>
                    </b-card-body>
                </b-card>        
            </b-col>
            <b-col sm="6" class="col-xl-6 col-lg-6 col-md-6 col-sm-12">
                <b-card no-body header="<b>Transactions</b>">
                    <b-list-group flush>
                    <b-list-group-item href="#">Cras justo odio</b-list-group-item>
                    <b-list-group-item href="#">Dapibus ac facilisis in</b-list-group-item>
                    <b-list-group-item href="#">Vestibulum at eros</b-list-group-item>
                    </b-list-group>

                    <b-card-body>
                    <b-button block variant="outline-primary">View more</b-button>
                    </b-card-body>
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

body {
    font-size: 0.9rem;
}

.btn {
    font-size: 0.8rem;
}
</style>