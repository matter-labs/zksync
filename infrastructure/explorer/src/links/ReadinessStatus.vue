<template>
    <span class="mr-1">
        <i v-if="status == Readiness.Rejected" class="fas fa-times brown"></i>
        <b-spinner v-else-if="status == Readiness.Initiated" small label="Small Spinner" />
        <img v-else-if="status == Readiness.Committed" small class="readiness-image" src="../assets/2.svg" />
        <img v-else-if="status == Readiness.Scheduled" small class="readiness-image" src="../assets/3.svg" />
        <img v-else-if="status == Readiness.Verified" small class="readiness-image" src="../assets/1.svg" />
    </span>
</template>

<script>
import { Readiness } from '../Readiness';

export default {
    name: 'ReadinessStatus',
    props: ['status'],
    data() {
        return {
            Readiness
        };
    }
};
</script>

<style scoped>
.readiness-image {
    height: 1.33em;
    margin-right: -0.25em;
}
.brown {
    color: #aa935d;
}
</style>
