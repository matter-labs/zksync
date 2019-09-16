<template>
    <b-alert dismissible v-bind:variant="variant" fade :show="countdown" @dismissed="dismissed" class="mt-2">
        {{ message }}
    </b-alert>
</template>

<script>
export default {
    name: 'Alert',
    data: () => ({
        countdown: 0,
        message: '',
        variant: 'info'
    }),
    methods: {
        dismissed() {
            this.message = '';
            this.countdown = 0;
        },
        display(kwargs) {
            this.message = kwargs.message;
            this.variant = kwargs.variant || this.variant;
            this.countdown = kwargs.countdown || 10;
        }
    }
}
</script>
