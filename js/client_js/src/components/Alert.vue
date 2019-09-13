<template>
    <b-alert dismissible variant="warning" fade :show="countdown" @dismissed="dismissed" class="mt-2">
        {{ message }}
    </b-alert>
</template>

<script>
export default {
    name: 'Alert',
    data: () => ({
        countdown: 0,
        message: '',
    }),
    methods: {
        dismissed() {
            this.message = '';
            this.countdown = 0;
        },
        display(kwargs) {
            this.message = kwargs.message;
            this.countdown = kwargs.countdown || 10;
        }
    }
}
</script>
