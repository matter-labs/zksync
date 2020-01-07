<template>
    <router-view></router-view>
</template>

<script>

export default {
    name: 'app',
}
</script>

<style lang="scss">
input::placeholder {
    color: lightgrey !important;
    font-style: italic !important;
}
@import "../../client/scss/common.scss";
</style>

<style lang="scss">
@import "../scss/common.scss";
</style>
