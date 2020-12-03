<template>
    <keep-alive>
        <router-view></router-view>
    </keep-alive>
</template>

<script>
export default {
    name: "app",
};
</script>

<style lang="scss">
@import "../scss/common.scss";
input::placeholder {
    color: lightgrey !important;
    font-style: italic !important;
}
table {
    font-size: 1.1em;
}
</style>
