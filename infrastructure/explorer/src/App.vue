<template>
    <div>
        <Navbar />
        <keep-alive max="10" include="home,tokens">
            <router-view></router-view>
        </keep-alive>
    </div>
</template>

<script>
import Navbar from './Navbar.vue';
export default {
    components: { Navbar },
    name: 'app'
};
</script>

<style lang="scss">
@import '../scss/common.scss';
input::placeholder {
    color: lightgrey !important;
    font-style: italic !important;
}
table {
    font-size: 1.1em;
}
</style>
