<template>
    <router-view v-if="store.config"></router-view>
</template>

<script>

export default {
    name: 'app',
}
</script>

<style>
input::placeholder {
    color: lightgrey !important;
    font-style: italic !important;
}
</style>