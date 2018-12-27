<template>
    <div id="app">
        <h1>Welcome to Plasma Wallet</h1>
        <h2>{{ counter }}</h2>
        <b-button @click="counter++">Click</b-button>
    </div>
</template>

<script>
export default {
    name: 'app',
    data () {
        return {
            counter: 0,
        }
    },
    created() {
        this.counter += 2
    },
}
</script>

<style>
#app {
    font-family: 'Avenir', Helvetica, Arial, sans-serif;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    text-align: center;
    color: #2c3e50;
    margin-top: 60px;
}

h1, h2 {
    font-weight: normal;
}

</style>
