<template>
    <!-- <img class="loading-image m-0 p-0" src="./assets/loading.gif" /> -->
    <span class="mr-1">
        <i v-if="getStatus == -1" class="fas fa-times brown"></i>
        <b-spinner v-else-if="getStatus == 0" small label="Small Spinner" />
        <img v-else-if="getStatus == 1" small class="readiness-image" src="./assets/2.svg" />
        <img v-else-if="getStatus == 2" small class="readiness-image" src="./assets/1.svg" />
    </span>
</template>

<script>
export default {
    name: 'ReadinessStatus',
    props: ['status'],
    computed: {
        getStatus() {
            if ([-1].includes(this.status)) return -1;
            if ([0].includes(this.status)) return 0;
            if ([1].includes(this.status)) return 1;
            if ([2].includes(this.status)) return 2;
            throw new Error('Invalid status');
        },
    },
};
</script>

<style scoped>
.readiness-image {
    height: 1.33em;
    margin-right: -0.25em;
}
.brown {
    color: #AA935D;
}
</style>
