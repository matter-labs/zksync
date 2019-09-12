<template>
    <b-alert v-if="message" dismissible variant="warning" fade :show="countdown" @dismissed="countdown=0" class="mt-2">
        {{ message }}
    </b-alert>
</template>

<script>
export default {
    name: 'Alert',
    props: ['message'],
    data: () => ({
        countdown: 0,
    }),
    watch: {
        message: function() {
            this.countdown = 10;
        }
    },
}
</script>
